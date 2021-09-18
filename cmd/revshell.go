package cmd

import (
	"os/user"
	"path/filepath"

	"github.com/ferama/rospo/cmd/cmnflags"
	"github.com/ferama/rospo/pkg/conf"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/ferama/rospo/pkg/sshd"
	"github.com/ferama/rospo/pkg/tun"
	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(revshellCmd)

	usr, _ := user.Current()
	defaultIdentity := filepath.Join(usr.HomeDir, ".ssh", "id_rsa")
	knownHostFile := filepath.Join(usr.HomeDir, ".ssh", "known_hosts")

	// sshc options
	revshellCmd.Flags().BoolP("insecure", "i", false, "disable known_hosts key server verification")
	revshellCmd.Flags().StringP("jump-host", "j", "", "optional jump host conf")
	revshellCmd.Flags().StringP("user-identity", "s", defaultIdentity, "the ssh identity (private) key absolute path")
	revshellCmd.Flags().StringP("known-hosts", "k", knownHostFile, "the known_hosts file absolute path")

	// tun options
	revshellCmd.Flags().StringP("remote", "r", "127.0.0.1:2222", "the remote shell listener endpoint")

	// sshd options
	cmnflags.AddSshdFlags(revshellCmd.Flags())
}

var revshellCmd = &cobra.Command{
	Use:   "revshell [user@]host[:port]",
	Short: "Starts a reverse shell",
	Args:  cobra.MinimumNArgs(1),
	Long:  "Starts a local sshd and forwards its port to the remote host",
	Example: `
  $ rospo revshell user@server	
	`,
	Run: func(cmd *cobra.Command, args []string) {
		sshdConf := cmnflags.GetSshDConf(cmd)
		s := sshd.NewSshServer(sshdConf)
		go s.Start()

		remote, _ := cmd.Flags().GetString("remote")
		jumpHost, _ := cmd.Flags().GetString("jump-host")
		identity, _ := cmd.Flags().GetString("user-identity")
		knownHosts, _ := cmd.Flags().GetString("known-hosts")
		insecure, _ := cmd.Flags().GetBool("insecure")

		config := &conf.Config{
			SshClient: &sshc.SshClientConf{
				Identity:   identity,
				KnownHosts: knownHosts,
				ServerURI:  args[0],
				JumpHosts:  make([]*sshc.JumpHostConf, 0),
				Insecure:   insecure,
			},
			Tunnel: []*tun.TunnelConf{
				{
					Remote:  remote,
					Local:   sshdConf.ListenAddress,
					Forward: false,
				},
			},
		}

		if jumpHost != "" {
			config.SshClient.JumpHosts = append(config.SshClient.JumpHosts, &sshc.JumpHostConf{
				URI:      jumpHost,
				Identity: identity,
			})
		}

		client := sshc.NewSshConnection(config.SshClient)
		go client.Start()

		tun.NewTunnel(client, config.Tunnel[0], false).Start()
	},
}
