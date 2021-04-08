package utils

import (
	"flag"
	"fmt"
	"os"
	"os/user"
	"path/filepath"
)

// Flags ...
type Flags struct {
	//// tunnell

	// args
	Username       *string
	ServerEndpoint *string
	JumpHost       *string

	UserIdentity   *string
	DisableTun     *bool
	LocalEndpoint  *string
	RemoteEndpoint *string
	Forward        *bool

	//// sshd
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
		JumpHost:             flag.String("jump-host", "", "Optional jump host conf"),
		ServerAuthorizedKeys: flag.String("server-authorized-keys", "./authorized_keys", "The ssh server authorized keys path"),
		SshdPort:             flag.String("sshd-port", "2222", "The ssh server tcp port"),
		LocalEndpoint:        flag.String("local", "127.0.0.1:2222", "The local endpoint"),
		RemoteEndpoint:       flag.String("remote", "127.0.0.1:5555", "The remote endpoint"),
		Forward:              flag.Bool("forward", false, "forwards a remote port to local"),
		DisableSshd:          flag.Bool("no-sshd", false, "If set disable the embedded ssh server"),
		DisableTun:           flag.Bool("no-tun", false, "If set disable the tunnel (starts the sshd service only)"),
	}

	flag.Parse()

	if !*flagValues.DisableTun {
		values := flag.Args()

		if len(values) == 0 {
			fmt.Printf("Usage: %s [user@]server[:port]\n", execName)
			flag.PrintDefaults()
			os.Exit(1)
		}
		parsed := ParseSSHUrl(values[0])
		flagValues.Username = &parsed.Username
		flagValues.ServerEndpoint = &parsed.Host
	}

	return flagValues
}
