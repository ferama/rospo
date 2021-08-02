package cmd

import (
	"github.com/ferama/rospo/pkg/sshd"

	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(sshdCmd)

	sshdCmd.Flags().StringP("sshd-authorized-keys", "K", "./authorized_keys", "ssh server authorized keys path")
	sshdCmd.Flags().StringP("sshd-listen-address", "P", ":2222", "the ssh server listen address")
	sshdCmd.Flags().StringP("sshd-key", "I", "./server_key", "the ssh server key path")
	sshdCmd.Flags().BoolP("disable-shell", "D", false, "if set disable shell/exec")
}

var sshdCmd = &cobra.Command{
	Use:   "sshd",
	Short: "Starts the sshd server",
	Long:  `Starts the sshd server`,
	Run: func(cmd *cobra.Command, args []string) {
		sshdKey, _ := cmd.Flags().GetString("sshd-key")
		sshdAuthorizedKeys, _ := cmd.Flags().GetString("sshd-authorized-keys")
		sshdListenAddress, _ := cmd.Flags().GetString("sshd-listen-address")
		disableShell, _ := cmd.Flags().GetBool("disable-shell")

		config := &sshd.SshDConf{
			Key:                sshdKey,
			AuthorizedKeysFile: sshdAuthorizedKeys,
			ListenAddress:      sshdListenAddress,
			DisableShell:       disableShell,
		}
		sshd.NewSshServer(config).Start()
	},
}
