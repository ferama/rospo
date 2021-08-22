package logger

import (
	"fmt"
	"io"
	"log"
	"os"
	"runtime"

	"golang.org/x/term"
)

const (
	Red     = "\033[0;31m"
	Green   = "\033[0;32m"
	Yellow  = "\033[0;33m"
	Blue    = "\033[0;34m"
	Magenta = "\033[0;35m"
	Cyan    = "\033[0;36m"
	White   = "\033[0;37m"
	reset   = "\033[0m"
)

var instances []*log.Logger

// DisableLoggers prevents any log output to be printed on console
func DisableLoggers() {
	for _, v := range instances {
		v.SetOutput(io.Discard)
	}
}

// NewLogger builds up and return a new logger
func NewLogger(prefix string, color string) *log.Logger {
	var logger *log.Logger
	if term.IsTerminal(int(os.Stdout.Fd())) && runtime.GOOS != "windows" {
		logger = log.New(os.Stdout, fmt.Sprintf("%s%s%s", color, prefix, reset), log.LstdFlags)
	} else {
		logger = log.New(os.Stdout, prefix, log.LstdFlags)
	}
	instances = append(instances, logger)
	return logger
}
