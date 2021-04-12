package rpty

import (
	"io"
	"os/exec"
)

// Pty pseudo-tty interface
type Pty interface {
	Resize(cols uint16, rows uint16) error
	Close() error
	Run(c *exec.Cmd) error
	WriteTo(io.Writer) (int64, error)
	ReadFrom(io.Reader) (int64, error)
}

// New creates a new Pty.
func New() (Pty, error) {
	return newPty()
}
