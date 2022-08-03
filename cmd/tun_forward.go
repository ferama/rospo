package cmd

import (
	"github.com/ferama/rospo/cmd/cmnflags"
	"github.com/ferama/rospo/pkg/conf"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/ferama/rospo/pkg/tun"

	"github.com/spf13/cobra"
)

func init() {
	tunCmd.AddCommand(tunForwardCmd)
}

var tunForwardCmd = &cobra.Command{
	Use:   "forward [user@][server]:port",
	Short: "Creates a forward ssh tunnel",
	Long: `Creates a forward ssh tunnel

Preliminary checks:
  1. Your remote server pubkey should be present into known_host file (disable this behaviour using the insecure flag)
     You can explicitly grab it with the 'grabpubkey' command
  2. Your identity should be authorized into the remote server (you can generate a new identity with the keygen comand)
`,
	Example: `
  # Forwards the local 8080 port to the remote 8080 
  $ rospo tun forward -l :8080 -r :8080 user@server:port
	`,
	Args: cobra.MinimumNArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		local, _ := cmd.Flags().GetString("local")
		remote, _ := cmd.Flags().GetString("remote")

		sshcConf := cmnflags.GetSshClientConf(cmd, args[0])
		config := &conf.Config{
			SshClient: sshcConf,
			Tunnel: []*tun.TunnelConf{
				{
					Remote:  remote,
					Local:   local,
					Forward: true,
				},
			},
		}

		client := sshc.NewSshConnection(config.SshClient)
		go client.Start()
		tun.NewTunnel(client, config.Tunnel[0], false).Start()
	},
}
