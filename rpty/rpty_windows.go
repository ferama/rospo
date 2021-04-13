package rpty

import (
	"io"
	"log"
	"os/exec"
)

func newPty() (Pty, error) {
	pty, err := newConPty(80, 32)
	if err != nil {
		return nil, err
	}
	return pty, nil
}

type rconPty struct {
	cpty *ConPty
}

func newConPty(cols int16, rows int16) (*rconPty, error) {
	c := &rconPty{}

	return c, nil
}

func (c *rconPty) Resize(cols uint16, rows uint16) error {
	// TODO
	return nil
}

func (c *rconPty) Close() error {
	c.cpty.Close()
	return nil
}

func (c *rconPty) Run(cm *exec.Cmd) error {
	// The Pty on windows is handled from
	// the conpty library. The subprocess it not
	// created directly using the os/exec go library
	// but using the windows.CreateProcess function instead
	// So here I'm going to take the cm.Path and pass it to the
	// ConPTYStart directly
	cpty, err := ConPTYStart(cm.Path)

	if err != nil {
		log.Fatalf("Failed to spawn a pty:  %v", err)
	}
	c.cpty = cpty

	return err
}

func (c *rconPty) WriteTo(dest io.Writer) (int64, error) {
	return io.Copy(dest, c.cpty)
}

func (c *rconPty) ReadFrom(src io.Reader) (int64, error) {
	return io.Copy(c.cpty, src)
}
