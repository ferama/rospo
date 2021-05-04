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

	// reads from pty and writes to io.Writeer
	WriteTo(io.Writer) (int64, error)

	// Reads from io.Reader and writes to pty
	ReadFrom(io.Reader) (int64, error)
}

// New creates a new Pty
func New() (Pty, error) {
	return newPty()
}
