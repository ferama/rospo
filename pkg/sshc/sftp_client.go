package sshc

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sync"

	"github.com/ferama/rospo/pkg/worker"
	"github.com/pkg/sftp"
)

type ProgressFunc func(fileSize int64, offset int64, fileName string, progressChan chan int64)

type SftpClient struct {
	sshConn   *SshConnection
	terminate chan bool
	connected sync.WaitGroup

	Client *sftp.Client
}

func NewSftpClient(sshConnection *SshConnection) *SftpClient {
	s := &SftpClient{
		sshConn:   sshConnection,
		terminate: make(chan bool, 1),
	}
	s.connected.Add(1)
	return s
}

// Waits until the connection is estabilished with the server
func (s *SftpClient) ReadyWait() {
	s.connected.Wait()
}

func (s *SftpClient) waitForSshClient() bool {
	c := make(chan bool)
	go func() {
		defer close(c)
		// WARN: if I have issues with sshConn this will wait forever
		s.sshConn.ReadyWait()
	}()
	select {
	case <-s.terminate:
		return false
	default:
		select {
		case <-c:
			return true
		case <-s.terminate:
			return false
		}
	}
}

func (s *SftpClient) Start() {
	for {
		for {
			if s.waitForSshClient() {
				log.Println("ssh client ready")
				break
			} else {
				log.Println("terminated")
				return
			}
		}
		client, err := sftp.NewClient(s.sshConn.Client)
		if err != nil {
			log.Printf("cannot create SFTP client: %s", err)
		} else {
			log.Println("SFTP client created")
			s.Client = client
			s.connected.Done()
			client.Wait()
			log.Println("SFTP connection lost")
			s.connected.Add(1)
		}
	}
}

func (s *SftpClient) Stop() {
	close(s.terminate)
}

func (s *SftpClient) GetFile(remote, localPath string, maxWorkers int, progressFn *ProgressFunc) error {
	const chunkSize = 128 * 1024 // 128KB per chunk

	s.ReadyWait()

	client := s.Client
	remotePath, err := client.RealPath(remote)
	if err != nil {
		return fmt.Errorf("invalid remote path: %s", remotePath)
	}
	remoteStat, err := client.Stat(remotePath)
	if err != nil {
		return fmt.Errorf("cannot stat remote path: %s", remotePath)
	}
	fileSize := remoteStat.Size()

	rFile, err := client.Open(remotePath)
	if err != nil {
		return fmt.Errorf("cannot open remote file for read: %s", err)
	}
	defer rFile.Close()

	// Handle directory case
	localStat, err := os.Stat(localPath)
	if err == nil && localStat.IsDir() {
		localPath = filepath.Join(localPath, filepath.Base(remotePath))
	}

	// Determine offset for resuming
	var offset int64
	lFile, err := os.OpenFile(localPath, os.O_CREATE|os.O_WRONLY, 0644)
	if err == nil {
		offset, _ = lFile.Seek(0, io.SeekEnd)
		lFile.Close()
	} else {
		offset = 0
	}

	// If the file is already fully downloaded, return early
	if offset >= fileSize {
		fmt.Println("File already fully downloaded.")
		return nil
	}

	// Reopen local file for writing
	lFile, err = os.OpenFile(localPath, os.O_CREATE|os.O_WRONLY, 0644)
	if err != nil {
		return fmt.Errorf("cannot open local file for write: %s", err)
	}
	defer lFile.Close()

	// Start progress bar
	progressCh := make(chan int64, maxWorkers)
	if progressFn != nil {
		go (*progressFn)(fileSize, offset, remotePath, progressCh)
	} else {
		go func() {
			for range progressCh {
			}
		}()
	}

	workerPool := worker.NewPool(maxWorkers)
	defer workerPool.Stop()

	// Enqueue only the remaining chunks for workers
	for chunkOffset := offset; chunkOffset < fileSize; chunkOffset += chunkSize {
		workerPool.Enqueue(func() {
			for {
				err := s.downloadChunk(remotePath, localPath, chunkOffset, chunkSize, progressCh)
				if err == nil {
					break // Success, move to next chunk
				}
			}
		})
	}

	workerPool.Wait()
	close(progressCh)

	// Set final file permissions
	return lFile.Chmod(remoteStat.Mode())
}

func (s *SftpClient) downloadChunk(remotePath string, localPath string, offset, chunkSize int64, progressCh chan<- int64) error {
	s.ReadyWait()

	buf := make([]byte, chunkSize)

	// Open remote file
	client := s.Client
	rFile, err := client.Open(remotePath)
	if err != nil {
		return fmt.Errorf("cannot open remote file for read: %s", err)
	}
	defer rFile.Close()

	// Seek to correct position in remote file
	if _, err := rFile.Seek(offset, io.SeekStart); err != nil {
		return fmt.Errorf("cannot seek remote file: %s", err)
	}

	// Read chunk
	totalRead := 0
	for totalRead < len(buf) {
		n, err := rFile.Read(buf[totalRead:])
		if err != nil && err != io.EOF {
			return fmt.Errorf("error reading remote file: %s", err)
		}
		if n == 0 {
			break
		}
		totalRead += n
	}

	lFile, err := os.OpenFile(localPath, os.O_CREATE|os.O_WRONLY, 0644)
	if err != nil {
		return fmt.Errorf("cannot open local file for write: %s", err)
	}
	defer lFile.Close()

	// Seek to correct position in local file
	if _, err := lFile.Seek(offset, io.SeekStart); err != nil {
		return fmt.Errorf("cannot seek local file: %s", err)
	}

	// Write chunk to local file
	totalWritten := 0
	for totalWritten < totalRead {
		written, err := lFile.Write(buf[totalWritten:totalRead])
		if err != nil {
			return fmt.Errorf("error writing local file: %s", err)
		}
		totalWritten += written
	}

	// Update progress
	progressCh <- int64(totalWritten)
	return nil
}

// Upload Chunk
func (s *SftpClient) uploadChunk(remotePath string, lFile *os.File, offset, chunkSize int64, progressCh chan<- int64) error {
	s.ReadyWait()

	buf := make([]byte, chunkSize)

	// Read chunk from local file
	n, err := lFile.ReadAt(buf, offset)
	if err != nil && err != io.EOF {
		return fmt.Errorf("error reading local file: %s", err)
	}

	// Open remote file for writing
	rFile, err := s.Client.OpenFile(remotePath, os.O_WRONLY|os.O_CREATE)
	if err != nil {
		return fmt.Errorf("cannot open remote file for write: %s", err)
	}
	defer rFile.Close()

	// Seek to correct position
	if _, err := rFile.Seek(offset, io.SeekStart); err != nil {
		return fmt.Errorf("cannot seek remote file: %s", err)
	}

	// Write chunk
	totalWritten := 0
	for totalWritten < n {
		written, err := rFile.Write(buf[totalWritten:n])
		if err != nil {
			return fmt.Errorf("error writing remote file: %s", err)
		}
		totalWritten += written
	}

	progressCh <- int64(totalWritten)
	return nil
}

func (s *SftpClient) PutFile(remote, localPath string, maxWorkers int, progressFn *ProgressFunc) error {
	const chunkSize = 128 * 1024 // 128KB per chunk

	s.ReadyWait()

	remotePath, err := s.Client.RealPath(remote)
	if err != nil {
		return fmt.Errorf("invalid remote path: %s", remotePath)
	}
	log.Println("remotePath", remotePath)

	localStat, err := os.Stat(localPath)
	if err != nil {
		return fmt.Errorf("cannot stat local path: %s", localPath)
	}
	fileSize := localStat.Size()

	lFile, err := os.Open(localPath)
	if err != nil {
		return fmt.Errorf("cannot open local file: %s", err)
	}
	defer lFile.Close()

	// Check if remote file already exists and determine resume offset
	var offset int64 = 0
	if remoteStat, err := s.Client.Stat(remotePath); err == nil {
		offset = remoteStat.Size()
	}

	if offset >= fileSize {
		log.Println("File already fully uploaded.")
		return nil
	}

	progressCh := make(chan int64, maxWorkers)

	if progressFn != nil {
		go (*progressFn)(fileSize, offset, remotePath, progressCh)
	} else {
		go func() {
			for range progressCh {
			}
		}()
	}

	log.Printf("Using %d workers", maxWorkers)
	workerPool := worker.NewPool(maxWorkers)
	defer workerPool.Stop()

	// Enqueue only the remaining chunks for workers
	for chunkOffset := offset; chunkOffset < fileSize; chunkOffset += chunkSize {
		workerPool.Enqueue(func() {
			for {
				err := s.uploadChunk(remotePath, lFile, chunkOffset, chunkSize, progressCh)
				if err == nil {
					break // Success, move to next chunk
				}
			}
		})
	}

	workerPool.Wait()
	close(progressCh)

	return s.Client.Chmod(remotePath, localStat.Mode())
}
