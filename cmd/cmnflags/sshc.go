package cmnflags

import (
	"fmt"
	"path/filepath"

	"github.com/ferama/rospo/pkg/sshc"
	"github.com/ferama/rospo/pkg/utils"
	"github.com/spf13/cobra"
	"github.com/spf13/pflag"
)

// AddSshClientFlags adds sshc common flags to FlagSet
func AddSshClientFlags(fs *pflag.FlagSet) {

	usr := utils.CurrentUser()
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
	sshURI := serverURI

	identity, _ := cmd.Flags().GetString("user-identity")
	knownHosts, _ := cmd.Flags().GetString("known-hosts")
	insecure, _ := cmd.Flags().GetBool("insecure")
	jumpHost, _ := cmd.Flags().GetString("jump-host")
	password, _ := cmd.Flags().GetString("password")

	disableBanner, _ := cmd.Flags().GetBool("disable-banner")

	cp := utils.GetSSHConfigInstance()
	hostConf := cp.GetHostConf(serverURI)
	if hostConf != nil {
		identity = hostConf.IdentityFile
		knownHosts = hostConf.UserKnownHostsFile
		if hostConf.ProxyJump != "" {
			jumpHost = hostConf.ProxyJump
		}
		insecure = !hostConf.StrictHostKeyChecking
		sshURI = fmt.Sprintf("%s@%s:%d", hostConf.User, hostConf.HostName, hostConf.Port)
	}

	sshcConf := &sshc.SshClientConf{
		Identity:   identity,
		KnownHosts: knownHosts,
		Password:   password,
		Quiet:      disableBanner,
		ServerURI:  sshURI,
		JumpHosts:  make([]*sshc.JumpHostConf, 0),
		Insecure:   insecure,
	}

	// search for jump hosts in the ssh config file
	if jumpHost != "" {
		for {
			jumpHostConf := cp.GetHostConf(jumpHost)
			if jumpHostConf == nil {
				break
			}
			identity = jumpHostConf.IdentityFile
			sshcConf.JumpHosts = append(sshcConf.JumpHosts, &sshc.JumpHostConf{
				URI:      fmt.Sprintf("%s@%s:%d", jumpHostConf.User, jumpHostConf.HostName, jumpHostConf.Port),
				Identity: identity,
			})
			jumpHost = jumpHostConf.ProxyJump
		}
	}

	// add jump host from command line
	if jumpHost != "" {
		sshcConf.JumpHosts = append(sshcConf.JumpHosts, &sshc.JumpHostConf{
			URI:      jumpHost,
			Identity: identity,
		})
	}

	// utils.PrettyPrintStruct(sshcConf)

	return sshcConf
}
