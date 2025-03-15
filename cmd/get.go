package cmd

import (
	"fmt"
	"io"
	"log"
	"os"
	"path/filepath"
	"strings"
	"sync"

	pb "github.com/cheggaaa/pb/v3"
	"github.com/ferama/rospo/cmd/cmnflags"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(getCmd)

	cmnflags.AddSshClientFlags(getCmd.Flags())
	getCmd.Flags().BoolP("recursive", "r", false, "if the copy should be recursive")
}

func getFile(sftpConn *sshc.SftpConnection, remote, localPath string) error {
	const chunkSize = 128 * 1024 // 128KB per chunk
	const maxWorkers = 8         // Number of parallel workers

	sftpConn.ReadyWait()

	client := sftpConn.Client
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
	go func() {
		tmpl := `{{string . "target" | white}} {{counters . | blue }} {{bar . "|" "=" ">" "." "|" }} {{percent . | blue }} {{speed . | blue }} {{rtime . "ETA %s" | blue }}`
		pbar := pb.ProgressBarTemplate(tmpl).Start64(fileSize)
		pbar.Set(pb.Bytes, true)
		pbar.Set(pb.SIBytesPrefix, true)
		pbar.Set("target", filepath.Base(remotePath))
		pbar.Add64(offset)
		for w := range progressCh {
			pbar.Add64(w)
		}
		pbar.Finish()
	}()

	// Job queue and worker synchronization
	var wg sync.WaitGroup
	jobs := make(chan int64, maxWorkers)

	// Worker Goroutines
	for i := range maxWorkers {
		wg.Add(1)
		go func(workerID int) {
			defer wg.Done()
			for chunkOffset := range jobs {
				for {
					sftpConn.ReadyWait()
					err := downloadChunk(sftpConn, remotePath, lFile, chunkOffset, chunkSize, progressCh)
					if err == nil {
						break // Success, move to next chunk
					}
				}
			}
		}(i)
	}

	// Enqueue only the remaining chunks for workers
	for chunkOffset := offset; chunkOffset < fileSize; chunkOffset += chunkSize {
		jobs <- chunkOffset
	}
	close(jobs)

	// Wait for all workers to complete
	wg.Wait()
	close(progressCh)

	// Set final file permissions
	return lFile.Chmod(remoteStat.Mode())
}

func downloadChunk(sftpConn *sshc.SftpConnection, remotePath string, lFile *os.File, offset, chunkSize int64, progressCh chan<- int64) error {
	buf := make([]byte, chunkSize)

	// Open remote file
	client := sftpConn.Client
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
			if isConnectionError(err) {
				return fmt.Errorf("connection lost")
			}
			return fmt.Errorf("error reading remote file: %s", err)
		}
		if n == 0 {
			break
		}
		totalRead += n
	}

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

func getFileRecursive(sftpConn *sshc.SftpConnection, remote, local string) error {
	remotePath, err := sftpConn.Client.RealPath(remote)
	if err != nil {
		return fmt.Errorf("invalid remote path: %s", remotePath)
	}

	remoteStat, err := sftpConn.Client.Stat(remotePath)
	if err != nil {
		return fmt.Errorf("cannot stat remote path: %s", remotePath)
	}
	if !remoteStat.IsDir() {
		return fmt.Errorf("remote path is not a directory: %s", remotePath)
	}

	localStat, err := os.Stat(local)
	if err != nil {
		return fmt.Errorf("cannot stat local path: %s", local)
	}
	if !localStat.IsDir() {
		return fmt.Errorf("local path is not a directory: %s", local)
	}

	dir := filepath.Dir(remotePath)
	walker := sftpConn.Client.Walk(remotePath)
	for walker.Step() {
		if walker.Err() != nil {
			log.Println(walker.Err())
			continue
		}
		remotePath := walker.Path()
		stat := walker.Stat()
		part := strings.TrimPrefix(remotePath, dir)
		localPath := filepath.Join(local, part)
		if stat.IsDir() {
			err := os.Mkdir(localPath, stat.Mode())
			if err != nil {
				return fmt.Errorf("cannot create directory %s: %s", localPath, err)
			}
		} else {
			getFile(sftpConn, remotePath, localPath)
		}
	}
	return nil
}

var getCmd = &cobra.Command{
	Use:   "get [user@]host[:port] remote [local]",
	Short: "Gets a file from remote",
	Long:  "Gets a file from remote",
	Example: `
  # downloads a file from the remote server
  $ rospo get myserver:2222 file.txt .

  # downloads recursively all contents of myremotefolder to local current working directory
  $ rospo get myserver:2222 /home/myuser/myremotefolder -r

  # downloads recursively all contents of myremotefolder to local target directory
  $ rospo get myserver:2222 /home/myserver/myremotefolder ~/mylocalfolder -r
	`,
	Args: cobra.MinimumNArgs(2),
	Run: func(cmd *cobra.Command, args []string) {
		remote := args[1]
		local := ""
		if len(args) > 2 {
			local = args[2]
		}
		recursive, _ := cmd.Flags().GetBool("recursive")
		sshcConf := cmnflags.GetSshClientConf(cmd, args[0])
		// sshcConf.Quiet = true
		conn := sshc.NewSshConnection(sshcConf)
		go conn.Start()

		sftpConn := sshc.NewSftpConnection(conn)
		go sftpConn.Start()

		if recursive {
			err := getFileRecursive(sftpConn, remote, local)
			if err != nil {
				log.Printf("error while copying file: %s", err)
			}
		} else {
			getFile(sftpConn, remote, local)
		}

	},
}
