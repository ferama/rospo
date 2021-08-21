package sshc

import (
	"os"
	"time"

	"golang.org/x/crypto/ssh"
	"golang.org/x/term"
)

// RemoteShell handles remote shell connections. It uses an ssh connection object
// and requests a shell inside a pty to the remote server
type RemoteShell struct {
	sshConn *SshConnection

	session *ssh.Session
	stopCh  chan bool
}

// NewRemoteShell creates a new RemoteShell object
func NewRemoteShell(sshConn *SshConnection) *RemoteShell {
	rs := &RemoteShell{
		sshConn: sshConn,
		stopCh:  make(chan bool),
	}
	return rs
}

// Start starts the remote shell
func (rs *RemoteShell) Start() {
	rs.sshConn.Connected.Wait()

	session, err := rs.sshConn.Client.NewSession()
	rs.session = session
	if err != nil {
		log.Fatalf("Failed to create session: " + err.Error())
	}
	defer session.Close()

	session.Stdout = os.Stdout
	session.Stderr = os.Stderr
	session.Stdin = os.Stdin

	fd := int(os.Stdin.Fd())
	state, err := term.MakeRaw(fd)
	if err != nil {
		log.Printf("terminal make raw: %s", err)
	}
	defer func() {
		if term.IsTerminal(fd) {
			term.Restore(fd, state)
		}
	}()

	// terminal size poller
	go func() {
		for {
			select {
			case <-time.After(100 * time.Millisecond):
				w, h, _ := term.GetSize(fd)
				session.WindowChange(h, w)
			case <-rs.stopCh:
				return
			}
		}
	}()

	w, h, err := term.GetSize(fd)
	if err != nil {
		log.Printf("terminal get size: %s", err)
	}

	// Set up terminal modes
	modes := ssh.TerminalModes{
		ssh.ECHO:          1,
		ssh.TTY_OP_ISPEED: 14400,
		ssh.TTY_OP_OSPEED: 14400,
	}

	terminal := os.Getenv("TERM")
	if terminal == "" {
		terminal = "xterm-256color"
	}
	// Request pseudo terminal
	if err := session.RequestPty(terminal, h, w, modes); err != nil {
		log.Fatalf("request for pseudo terminal failed: %s", err)
	}

	// Start remote shell
	if err := session.Shell(); err != nil {
		log.Fatalf("failed to start shell: %s", err)
	}

	session.Wait()
}

// Stop stops the remote shell
func (rs *RemoteShell) Stop() {
	rs.stopCh <- true
	rs.session.Close()
}
