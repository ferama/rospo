package cmd

import (
	"strings"

	"github.com/ferama/rospo/cmd/cmnflags"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(shellCmd)

	cmnflags.AddSshClientFlags(shellCmd.Flags())
}

var shellCmd = &cobra.Command{
	Use:   "shell [user@]host[:port] [cmd_string]",
	Short: "Starts a remote shell",
	Long:  "Starts a remote shell",
	Args:  cobra.MinimumNArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		sshcConf := cmnflags.GetSshClientConf(cmd, args)
		conn := sshc.NewSshConnection(sshcConf)
		go conn.Start()

		remoteShell := sshc.NewRemoteShell(conn)
		remoteShell.Start(strings.Join(args[1:], " "), true)
	},
}
