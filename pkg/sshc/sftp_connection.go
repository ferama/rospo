package sshc

import (
	"sync"

	"github.com/pkg/sftp"
)

type SftpConnection struct {
	sshConn   *SshConnection
	terminate chan bool
	connected sync.WaitGroup

	Client *sftp.Client
}

func NewSftpConnection(sshConnection *SshConnection) *SftpConnection {
	s := &SftpConnection{
		sshConn:   sshConnection,
		terminate: make(chan bool, 1),
	}
	s.connected.Add(1)
	return s
}

// Waits until the connection is estabilished with the server
func (s *SftpConnection) ReadyWait() {
	s.connected.Wait()
}

func (s *SftpConnection) waitForSshClient() bool {
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

func (s *SftpConnection) Start() {
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

func (s *SftpConnection) Stop() {
	close(s.terminate)
}
