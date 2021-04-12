package sshd

import (
	"fmt"
	"io"
	"log"
	"os"
	"os/exec"
	"sync"
	"syscall"

	"github.com/creack/pty"
	"golang.org/x/crypto/ssh"
)

func ptyOpen() (ptyFile, tty *os.File, err error) {
	return pty.Open()
}

// Assigns a pseudo-terminal tty os.File to c.Stdin, c.Stdout,
// and c.Stderr, calls c.Start, and returns the File of the tty's
// corresponding pty.
func ptyRun(c *exec.Cmd, tty *os.File) (err error) {
	defer tty.Close()
	c.Stdout = tty
	c.Stdin = tty
	c.Stderr = tty
	c.SysProcAttr = &syscall.SysProcAttr{
		Setctty: true,
		Noctty:  false,
		Setsid:  true,
	}
	return c.Start()
}

func ptyServe(channel ssh.Channel, pty *os.File, cmd *exec.Cmd) {
	// Teardown session
	var once sync.Once
	close := func() {
		channel.Close()
		cmd.Process.Wait()
		log.Printf("[SSHD] session closed")
	}

	// Pipe session to shell and vice-versa
	go func() {
		_, err := io.Copy(channel, pty)
		if err != nil {
			log.Println(fmt.Sprintf("[SSHD] error while copy from channel: %s", err))
		}
		once.Do(close)
	}()

	go func() {
		_, err := io.Copy(pty, channel)
		if err != nil {
			log.Println(fmt.Sprintf("[SSHD] error while copy to channel: %s", err))
		}
		once.Do(close)
	}()
}
