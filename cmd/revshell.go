package cmd

import (
	"github.com/ferama/rospo/cmd/cmnflags"
	"github.com/ferama/rospo/pkg/conf"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/ferama/rospo/pkg/sshd"
	"github.com/ferama/rospo/pkg/tun"
	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(revshellCmd)

	// sshc options
	cmnflags.AddSshClientFlags(revshellCmd.Flags())

	// tun options
	revshellCmd.Flags().StringP("remote", "r", "127.0.0.1:2222", "the remote shell listener endpoint")

	// sshd options
	cmnflags.AddSshDFlags(revshellCmd.Flags())
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

		sshcConf := cmnflags.GetSshClientConf(cmd, args)
		config := &conf.Config{
			SshClient: sshcConf,
			Tunnel: []*tun.TunnelConf{
				{
					Remote:  remote,
					Local:   sshdConf.ListenAddress,
					Forward: false,
				},
			},
		}

		client := sshc.NewSshConnection(config.SshClient)
		go client.Start()

		tun.NewTunnel(client, config.Tunnel[0], false).Start()
	},
}
