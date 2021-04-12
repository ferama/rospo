package sshd

import (
	"log"
	"os"
	"os/exec"
	"syscall"

	"github.com/creack/pty"
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

func ptySetSize(tty *os.File, w, h uint32) {
	log.Printf("[SSHD] set window resize %dx%d", w, h)
	pty.Setsize(tty, &pty.Winsize{
		Rows: uint16(h),
		Cols: uint16(w),
	})
}
