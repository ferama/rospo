// +build !windows

package rpty

import (
	"io"
	"log"
	"os"
	"os/exec"
	"syscall"

	"github.com/creack/pty"
)

func newPty() (Pty, error) {
	pty, tty, err := pty.Open()
	if err != nil {
		return nil, err
	}

	return &nixPty{
		pty: pty,
		tty: tty,
	}, nil
}

type nixPty struct {
	pty, tty *os.File
	cmd      *exec.Cmd
}

func (p *nixPty) Resize(cols uint16, rows uint16) error {
	return pty.Setsize(p.tty, &pty.Winsize{
		Rows: rows,
		Cols: cols,
	})
}

func (p *nixPty) Close() error {
	if _, err := p.cmd.Process.Wait(); err != nil {
		log.Printf("[PTY] failed to exit process, killing. %s", err)
		p.cmd.Process.Kill()
	}
	p.pty.Close()
	p.tty.Close()
	return nil
}

func (p *nixPty) Run(c *exec.Cmd) error {
	defer p.tty.Close()

	p.cmd = c
	c.Stdout = p.tty
	c.Stdin = p.tty
	c.Stderr = p.tty
	c.SysProcAttr = &syscall.SysProcAttr{
		Setctty: true,
		Noctty:  false,
		Setsid:  true,
	}
	return c.Start()
}

func (p *nixPty) WriteTo(dest io.Writer) (int64, error) {
	return io.Copy(dest, p.pty)
}

func (p *nixPty) ReadFrom(src io.Reader) (int64, error) {
	return io.Copy(p.pty, src)
}
