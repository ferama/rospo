// +build windows

// This file was adapted from here
// https://github.com/UserExistsError/conpty
// It comes with
//
// MIT License
//
// Copyright (c) 2020 UserExistsError

package rpty

import (
	"fmt"
	"golang.org/x/sys/windows"
	"sync"
	"unsafe"
)

var (
	modKernel32                        = windows.NewLazySystemDLL("kernel32.dll")
	fCreatePseudoConsole               = modKernel32.NewProc("CreatePseudoConsole")
	fResizePseudoConsole               = modKernel32.NewProc("ResizePseudoConsole")
	fClosePseudoConsole                = modKernel32.NewProc("ClosePseudoConsole")
	fInitializeProcThreadAttributeList = modKernel32.NewProc("InitializeProcThreadAttributeList")
	fUpdateProcThreadAttribute         = modKernel32.NewProc("UpdateProcThreadAttribute")
)

func IsConPtyAvailable() bool {
	return fCreatePseudoConsole.Find() == nil &&
		fResizePseudoConsole.Find() == nil &&
		fClosePseudoConsole.Find() == nil &&
		fInitializeProcThreadAttributeList.Find() == nil &&
		fUpdateProcThreadAttribute.Find() == nil
}

const (
	STILL_ACTIVE                        uint32  = 259
	S_OK                                uintptr = 0
	PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE uintptr = 0x20016
)

type COORD struct {
	X, Y int16
}

func (c *COORD) Pack() uintptr {
	return uintptr((int32(c.Y) << 16) | int32(c.X))
}

type HPCON windows.Handle

type handleIO struct {
	handle windows.Handle
}

func (h *handleIO) Read(p []byte) (int, error) {
	var numRead uint32 = 0
	err := windows.ReadFile(h.handle, p, &numRead, nil)
	return int(numRead), err
}

func (h *handleIO) Write(p []byte) (int, error) {
	var numWritten uint32 = 0
	err := windows.WriteFile(h.handle, p, &numWritten, nil)
	return int(numWritten), err
}

func (h *handleIO) Close() error {
	return windows.CloseHandle(h.handle)
}

type ConPty struct {
	hpc                          HPCON
	pi                           *windows.ProcessInformation
	ptyIn, ptyOut, cmdIn, cmdOut *handleIO

	mu     sync.Mutex
	closed bool

	ready bool
}

func win32ClosePseudoConsole(hPc HPCON) {
	if fClosePseudoConsole.Find() != nil {
		return
	}
	fClosePseudoConsole.Call(uintptr(hPc))
}

func win32ResizePseudoConsole(hPc HPCON, coord *COORD) error {
	if fResizePseudoConsole.Find() != nil {
		return fmt.Errorf("ResizePseudoConsole not found")
	}
	ret, _, _ := fResizePseudoConsole.Call(uintptr(hPc), coord.Pack())
	if ret != S_OK {
		return fmt.Errorf("ResizePseudoConsole failed with status 0x%x", ret)
	}
	return nil
}

func win32CreatePseudoConsole(c *COORD, hIn, hOut windows.Handle) (HPCON, error) {
	if fCreatePseudoConsole.Find() != nil {
		return 0, fmt.Errorf("CreatePseudoConsole not found")
	}
	var hPc HPCON
	ret, _, _ := fCreatePseudoConsole.Call(
		c.Pack(),
		uintptr(hIn),
		uintptr(hOut),
		0,
		uintptr(unsafe.Pointer(&hPc)))
	if ret != S_OK {
		return 0, fmt.Errorf("CreatePseudoConsole() failed with status 0x%x", ret)
	}
	return hPc, nil
}

type StartupInfoEx struct {
	startupInfo   windows.StartupInfo
	attributeList []byte
}

func getStartupInfoExForPTY(hpc HPCON) (*StartupInfoEx, error) {
	if fInitializeProcThreadAttributeList.Find() != nil {
		return nil, fmt.Errorf("InitializeProcThreadAttributeList not found")
	}
	if fUpdateProcThreadAttribute.Find() != nil {
		return nil, fmt.Errorf("UpdateProcThreadAttribute not found")
	}
	var siEx StartupInfoEx
	siEx.startupInfo.Cb = uint32(unsafe.Sizeof(windows.StartupInfo{}) + unsafe.Sizeof(&siEx.attributeList[0]))
	var size uintptr

	// first call is to get required size. this should return false.
	ret, _, _ := fInitializeProcThreadAttributeList.Call(0, 1, 0, uintptr(unsafe.Pointer(&size)))
	siEx.attributeList = make([]byte, size, size)
	ret, _, err := fInitializeProcThreadAttributeList.Call(
		uintptr(unsafe.Pointer(&siEx.attributeList[0])),
		1,
		0,
		uintptr(unsafe.Pointer(&size)))
	if ret != 1 {
		return nil, fmt.Errorf("InitializeProcThreadAttributeList: %v", err)
	}

	ret, _, err = fUpdateProcThreadAttribute.Call(
		uintptr(unsafe.Pointer(&siEx.attributeList[0])),
		0,
		PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
		uintptr(hpc),
		unsafe.Sizeof(hpc),
		0,
		0)
	if ret != 1 {
		return nil, fmt.Errorf("InitializeProcThreadAttributeList: %v", err)
	}
	return &siEx, nil
}

func createConsoleProcessAttachedToPTY(hpc HPCON, commandLine string) (*windows.ProcessInformation, error) {
	cmdLine, err := windows.UTF16PtrFromString(commandLine)
	if err != nil {
		return nil, err
	}
	siEx, err := getStartupInfoExForPTY(hpc)
	if err != nil {
		return nil, err
	}
	var pi windows.ProcessInformation
	err = windows.CreateProcess(
		nil, // use this if no args
		cmdLine,
		nil,
		nil,
		false, // inheritHandle
		windows.EXTENDED_STARTUPINFO_PRESENT,
		nil,
		nil,
		&siEx.startupInfo,
		&pi)
	if err != nil {
		return nil, err
	}
	return &pi, nil
}

func closeHandles(handles ...windows.Handle) {
	for _, h := range handles {
		windows.CloseHandle(h)
	}
}

func (cpty *ConPty) Close() uint32 {
	// prevent panic from calling this method more then once
	// from diffent places
	cpty.mu.Lock()
	defer cpty.mu.Unlock()
	if cpty.closed {
		return 0
	}
	cpty.closed = true

	win32ClosePseudoConsole(cpty.hpc)
	cpty.ptyIn.Close()
	cpty.ptyOut.Close()
	cpty.cmdIn.Close()
	cpty.cmdOut.Close()
	var exitCode uint32 = STILL_ACTIVE
	windows.GetExitCodeProcess(cpty.pi.Process, &exitCode)
	return exitCode
}

func (cpty *ConPty) Wait() {
	for {
		ret, _ := windows.WaitForSingleObject(cpty.pi.Process, 1000)
		if ret != uint32(windows.WAIT_TIMEOUT) {
			break
		}
	}
}

func (cpty *ConPty) Read(p []byte) (int, error) {
	n, err := cpty.cmdOut.Read(p)
	return n, err
}

func (cpty *ConPty) Write(p []byte) (int, error) {
	n, err := cpty.cmdIn.Write(p)
	return n, err
}

func ConPTYStart(commandLine string) (*ConPty, error) {
	if !IsConPtyAvailable() {
		return nil, fmt.Errorf("ConPty is not available on this version of Windows")
	}

	var cmdIn, cmdOut, ptyIn, ptyOut windows.Handle
	if err := windows.CreatePipe(&ptyIn, &cmdIn, nil, 0); err != nil {
		return nil, fmt.Errorf("CreatePipe: %v", err)
	}
	if err := windows.CreatePipe(&cmdOut, &ptyOut, nil, 0); err != nil {
		closeHandles(ptyIn, cmdIn)
		return nil, fmt.Errorf("CreatePipe: %v", err)
	}

	coord := &COORD{80, 40}
	hPc, err := win32CreatePseudoConsole(coord, ptyIn, ptyOut)
	if err != nil {
		closeHandles(ptyIn, ptyOut, cmdIn, cmdOut)
		return nil, err
	}

	pi, err := createConsoleProcessAttachedToPTY(hPc, commandLine)
	if err != nil {
		closeHandles(ptyIn, ptyOut, cmdIn, cmdOut)
		win32ClosePseudoConsole(hPc)
		return nil, fmt.Errorf("Failed to create console process: %v", err)
	}

	cpty := &ConPty{
		hpc:    hPc,
		pi:     pi,
		ptyIn:  &handleIO{ptyIn},
		ptyOut: &handleIO{ptyOut},
		cmdIn:  &handleIO{cmdIn},
		cmdOut: &handleIO{cmdOut},

		closed: false,
	}

	go func() {
		// wait for process to finish then close the pipes
		defer cpty.Close()
		for {
			ret, _ := windows.WaitForSingleObject(cpty.pi.Process, 1000)
			if ret != uint32(windows.WAIT_TIMEOUT) {
				break
			}
		}
	}()

	return cpty, nil
}
