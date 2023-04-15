package cmnflags

import (
	"os/user"
	"path/filepath"

	"github.com/ferama/rospo/pkg/sshc"
	"github.com/spf13/cobra"
	"github.com/spf13/pflag"
)

// AddSshClientFlags adds sshc common flags to FlagSet
func AddSshClientFlags(fs *pflag.FlagSet) {

	usr, _ := user.Current()
	defaultIdentity := filepath.Join(usr.HomeDir, ".ssh", "id_rsa")
	knownHostFile := filepath.Join(usr.HomeDir, ".ssh", "known_hosts")

	fs.BoolP("disable-banner", "b", false, "if set disable server banner printing")
	fs.BoolP("insecure", "i", false, "disable known_hosts key server verification")
	fs.StringP("jump-host", "j", "", "optional jump host conf")
	fs.StringP("user-identity", "s", defaultIdentity, "the ssh identity (private) key absolute path")
	fs.StringP("known-hosts", "k", knownHostFile, "the known_hosts file absolute path")
	fs.StringP("password", "p", "", "the ssh client password")
}

// GetSshClientConf builds an SshcConf object from cmd
func GetSshClientConf(cmd *cobra.Command, serverURI string) *sshc.SshClientConf {
	identity, _ := cmd.Flags().GetString("user-identity")
	knownHosts, _ := cmd.Flags().GetString("known-hosts")
	insecure, _ := cmd.Flags().GetBool("insecure")
	jumpHost, _ := cmd.Flags().GetString("jump-host")
	password, _ := cmd.Flags().GetString("password")

	disableBanner, _ := cmd.Flags().GetBool("disable-banner")

	sshcConf := &sshc.SshClientConf{
		Identity:   identity,
		KnownHosts: knownHosts,
		Password:   password,
		Quiet:      disableBanner,
		ServerURI:  serverURI,
		JumpHosts:  make([]*sshc.JumpHostConf, 0),
		Insecure:   insecure,
	}
	if jumpHost != "" {
		sshcConf.JumpHosts = append(sshcConf.JumpHosts, &sshc.JumpHostConf{
			URI:      jumpHost,
			Identity: identity,
		})
	}

	return sshcConf
}
