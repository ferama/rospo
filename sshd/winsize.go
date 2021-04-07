package sshd

import (
	"log"
	"syscall"
	"unsafe"
)

// winsize stores the Height and Width of a terminal.
type winsize struct {
	Height uint16
	Width  uint16
	x      uint16 // unused
	y      uint16 // unused
}

// setWinsize sets the size of the given pty.
func setWinsize(fd uintptr, w, h uint32) {
	log.Printf("[SSHD] window resize %dx%d", w, h)
	ws := &winsize{Width: uint16(w), Height: uint16(h)}
	syscall.Syscall(syscall.SYS_IOCTL, fd, uintptr(syscall.TIOCSWINSZ), uintptr(unsafe.Pointer(ws)))
}
