package cmd

import (
	"github.com/ferama/rospo/cmd/cmnflags"
	"github.com/ferama/rospo/pkg/autocomplete"
	"github.com/ferama/rospo/pkg/conf"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/ferama/rospo/pkg/tun"

	"github.com/spf13/cobra"
)

func init() {
	tunCmd.AddCommand(tunReverseCmd)
}

var tunReverseCmd = &cobra.Command{
	Use:   "reverse [user@][server]:port",
	Short: "Creates a reverse ssh tunnel",
	Long: `Creates a reverse ssh tunnel

Preliminary checks:
  1. Your remote server pubkey should be present into known_host file (disable this behaviour using the insecure flag)
     You can explicitly grab it with the 'grabpubkey' command
  2. Your identity should be authorized into the remote server  (you can generate a new identity with the keygen comand)
`,
	Example: `
  # Start a reverse tunnel from the local port 5000 to the remote 8888
  # proxing through a jump host server
  $ rospo tun reverse -l :5000 -r :8888 -j jump_host_server user@server
	`,
	Args:              cobra.MinimumNArgs(1),
	ValidArgsFunction: autocomplete.Host(),
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
					Forward: false,
				},
			},
		}

		client := sshc.NewSshConnection(config.SshClient)
		go client.Start()
		// I can easily run multiple tunnels in their respective
		// go routine here using the same client
		tun.NewTunnel(client, config.Tunnel[0], false).Start()
	},
}
