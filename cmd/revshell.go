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
	Long: `Starts a local sshd and forwards its port to the remote host

Preliminary checks:
  1. Your remote server pubkey should be present into known_host file (disable this behaviour using the insecure flag)
     You can explicitly grab it with the 'grabpubkey' command
  2. Your identity should be authorized into the remote server  (you can generate a new identity with the keygen comand)
  3. You local authorized_keys file should be created and should contain at least a key to be able to reverse connect.
	 You can use have different options here, please check the "-K" flag
`,
	Example: `
  # starts a revshell at user@server at default remote address :2222
  $ rospo revshell user@server	

  # starts a revshell at user@server at remote address :6666
  $ rospo revshell -r :6666  -K http://github.com/[user].keys user@server
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
