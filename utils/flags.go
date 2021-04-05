package utils

import (
	"flag"
	"fmt"
	"os"
	"os/user"
	"path/filepath"
	"strings"
)

// Flags ...
type Flags struct {
	Identity       *string
	Username       *string
	LocalEndpoint  *string
	ServerEndpoint *string
	RemoteEndpoint *string
	Forward        *bool
}

var flagValues *Flags

// GetFlags ...
func GetFlags() *Flags {
	execName := os.Args[0]

	if flagValues != nil {
		return flagValues
	}

	usr, _ := user.Current()
	defaultIdentity := filepath.Join(usr.HomeDir, ".ssh", "id_rsa")

	flagValues = &Flags{
		Identity:       flag.String("identity", defaultIdentity, "The ssh identity (private) key absolute path"),
		LocalEndpoint:  flag.String("local", "localhost:22", "The local endpoint"),
		RemoteEndpoint: flag.String("remote", "localhost:5555", "The remote endpoint"),
		Forward:        flag.Bool("forward", true, "forwards a remote port to local"),
	}

	flag.Parse()
	values := flag.Args()

	if len(values) == 0 {
		fmt.Printf("Usage: %s user@server:port\n", execName)
		flag.PrintDefaults()
		os.Exit(1)
	}
	parts := strings.Split(values[0], "@")
	if len(parts) == 2 {
		flagValues.Username = &parts[0]
		flagValues.ServerEndpoint = &parts[1]
	} else {
		flagValues.ServerEndpoint = &parts[0]
	}

	return flagValues
}
