package cmd

import (
	"strings"

	"github.com/ferama/rospo/pkg/logger"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(shellCmd)
	shellCmd.Flags().BoolP("disable-banner", "b", false, "if set disable server banner printing")
}

var shellCmd = &cobra.Command{
	Use:   "shell [user@]host[:port] [cmd_string]",
	Short: "Starts a remote shell",
	Long:  "Starts a remote shell",
	Args:  cobra.MinimumNArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		logger.DisableLoggers()

		identity, _ := cmd.Flags().GetString("user-identity")
		knownHosts, _ := cmd.Flags().GetString("known-hosts")
		insecure, _ := cmd.Flags().GetBool("insecure")
		jumpHost, _ := cmd.Flags().GetString("jump-host")

		disableBanner, _ := cmd.Flags().GetBool("disable-banner")

		sshcConf := sshc.SshClientConf{
			Identity:   identity,
			KnownHosts: knownHosts,
			Quiet:      disableBanner,
			ServerURI:  args[0],
			JumpHosts:  make([]*sshc.JumpHostConf, 0),
			Insecure:   insecure,
		}

		if jumpHost != "" {
			sshcConf.JumpHosts = append(sshcConf.JumpHosts, &sshc.JumpHostConf{
				URI:      jumpHost,
				Identity: identity,
			})
		}

		conn := sshc.NewSshConnection(&sshcConf)
		go conn.Start()

		remoteShell := sshc.NewRemoteShell(conn)
		remoteShell.Start(strings.Join(args[1:], " "), true)
	},
}
