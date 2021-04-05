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
	UserIdentity   *string
	Username       *string
	LocalEndpoint  *string
	ServerEndpoint *string
	RemoteEndpoint *string
	Forward        *bool

	DisableSshd          *bool
	ServerIdentity       *string
	ServerAuthorizedKeys *string
	SshdPort             *string
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
		UserIdentity:         flag.String("user-identity", defaultIdentity, "The ssh identity (private) key absolute path"),
		ServerIdentity:       flag.String("server-identity", "./id_rsa", "The ssh server key path"),
		ServerAuthorizedKeys: flag.String("server-authorized-keys", "./authorized_keys", "The ssh server authorized keys path"),
		SshdPort:             flag.String("sshd-port", "2222", "The ssh server tcp port"),
		LocalEndpoint:        flag.String("local", "localhost:2222", "The local endpoint"),
		RemoteEndpoint:       flag.String("remote", "localhost:4444", "The remote endpoint"),
		Forward:              flag.Bool("forward", false, "forwards a remote port to local"),
		DisableSshd:          flag.Bool("no-sshd", false, "If set disable the embedded ssh server"),
	}

	flag.Parse()
	values := flag.Args()

	if len(values) == 0 {
		fmt.Printf("Usage: %s [user@]server[:port]\n", execName)
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
