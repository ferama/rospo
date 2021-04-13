package rpty

import (
	"io"
	"log"
	"os/exec"
	"time"
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
	// Very VERY hackish, but works
	// The problem here is that I could not have a cpty (ConPty)
	// ready when this function is called.
	// The cpty will not become ready until the Run function is called
	// There should be a better way to handle this one but I'm not
	// expert with windows API and I'm still trying to hack the conpty
	// library
	go func() {
		t := 3
		for {
			if c.cpty == nil {
				time.Sleep(1 * time.Second)
				t--
				if t == 0 {
					break
				}
			}
			win32ResizePseudoConsole(c.cpty.hpc, &COORD{
				X: int16(cols),
				Y: int16(rows),
			})
			break
		}
	}()
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
	// but using the windows.CreateProcess syscall instead
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
