package cmd

import (
	"rospo/tun"
	"rospo/utils"

	"github.com/spf13/cobra"
)

func init() {
	tunCmd.AddCommand(tunForwardCmd)
}

var tunForwardCmd = &cobra.Command{
	Use:   "forward [user@][server]:port",
	Short: "Creates a forward ssh tunnel",
	Long: `Creates a forward ssh tunnel
	
Example: 
  $ rospo tun forward -l :8080 -r :8080 user@server:port
	`,
	Args: cobra.MinimumNArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		local, _ := cmd.Flags().GetString("local")
		remote, _ := cmd.Flags().GetString("remote")
		jumpHost, _ := cmd.Flags().GetString("jump-host")
		identity, _ := cmd.Flags().GetString("user-identity")
		parsed := utils.ParseSSHUrl(args[0])

		tun.NewTunnel(
			parsed.Username,
			identity,
			tun.NewEndpoint(parsed.Host),
			tun.NewEndpoint(remote),
			tun.NewEndpoint(local),
			true,
			jumpHost,
		).Start()
	},
}
