// +build !windows

package rpty

import (
	"io"
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

	return &unixPty{
		pty: pty,
		tty: tty,
	}, nil
}

type unixPty struct {
	pty, tty *os.File
}

func (p *unixPty) Resize(cols uint16, rows uint16) error {
	return pty.Setsize(p.tty, &pty.Winsize{
		Rows: rows,
		Cols: cols,
	})
}

func (p *unixPty) Close() error {
	// p.pty.Close()
	p.tty.Close()
	return nil
}

func (p *unixPty) Run(c *exec.Cmd) error {
	defer p.Close()
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

func (p *unixPty) WriteTo(dest io.Writer) (int64, error) {
	return io.Copy(dest, p.pty)
}

func (p *unixPty) ReadFrom(src io.Reader) (int64, error) {
	return io.Copy(p.pty, src)
}
