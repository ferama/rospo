package cmd

import (
	"github.com/ferama/rospo/cmd/cmnflags"
	"github.com/ferama/rospo/pkg/sshd"

	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(sshdCmd)

	cmnflags.AddSshdFlags(sshdCmd.Flags())
	sshdCmd.Flags().BoolP("disable-shell", "D", false, "if set disable shell/exec")
}

var sshdCmd = &cobra.Command{
	Use:   "sshd",
	Short: "Starts the sshd server",
	Long:  `Starts the sshd server`,
	Run: func(cmd *cobra.Command, args []string) {
		disableShell, _ := cmd.Flags().GetBool("disable-shell")
		config := cmnflags.GetSshDConf(cmd)
		config.DisableShell = disableShell
		sshd.NewSshServer(config).Start()
	},
}
