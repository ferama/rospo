package logger

import (
	"fmt"
	"log"
	"os"

	"github.com/mattn/go-isatty"
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

// NewLogger builds up and return a new logger
func NewLogger(prefix string, color string) *log.Logger {
	var logger *log.Logger
	if isatty.IsTerminal(os.Stdout.Fd()) {
		logger = log.New(os.Stdout, fmt.Sprintf("%s%s%s", color, prefix, reset), log.LstdFlags)
	} else {
		logger = log.New(os.Stdout, prefix, log.LstdFlags)
	}
	return logger
}
