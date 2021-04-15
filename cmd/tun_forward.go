package cmd

import (
	"github.com/ferama/rospo/tun"
	"github.com/ferama/rospo/utils"

	"github.com/spf13/cobra"
)

func init() {
	tunCmd.AddCommand(tunForwardCmd)
}

var tunForwardCmd = &cobra.Command{
	Use:   "forward [user@][server]:port",
	Short: "Creates a forward ssh tunnel",
	Long:  `Creates a forward ssh tunnel`,
	Example: `
  # Forwards the local 8080 port to the remote 8080 
  $ rospo tun forward -l :8080 -r :8080 user@server:port
	`,
	Args: cobra.MinimumNArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		local, _ := cmd.Flags().GetString("local")
		remote, _ := cmd.Flags().GetString("remote")
		jumpHost, _ := cmd.Flags().GetString("jump-host")
		identity, _ := cmd.Flags().GetString("user-identity")
		insecure, _ := cmd.Flags().GetBool("insecure")
		parsed := utils.ParseSSHUrl(args[0])

		config := &tun.Config{
			Username: parsed.Username,
			Identity: identity,
			Server:   args[0],
			Remote:   remote,
			Local:    local,
			JumpHost: jumpHost,
			Forward:  true,
			Insecure: insecure,
		}

		tun.NewTunnel(config).Start()
	},
}
