package cmd

import (
	"github.com/ferama/rospo/conf"
	"github.com/ferama/rospo/sshd"

	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(sshdCmd)

	sshdCmd.Flags().StringP("sshd-authorized-keys", "K", "./authorized_keys", "ssh server authorized keys path")
	sshdCmd.Flags().StringP("sshd-port", "P", "2222", "the ssh server tcp port")
	sshdCmd.Flags().StringP("sshd-identity", "I", "./server_key", "the ssh server key path")
}

var sshdCmd = &cobra.Command{
	Use:   "sshd",
	Short: "Starts the sshd server",
	Long:  `Starts the sshd server`,
	Run: func(cmd *cobra.Command, args []string) {
		sshdIdentity, _ := cmd.Flags().GetString("sshd-identity")
		sshdAuthorizedKeys, _ := cmd.Flags().GetString("sshd-authorized-keys")
		sshdPort, _ := cmd.Flags().GetString("sshd-port")

		config := &conf.SshDConf{
			Identity:          sshdIdentity,
			AuthorizedKeyFile: sshdAuthorizedKeys,
			Port:              sshdPort,
		}
		sshd.NewSshServer(config).Start()
	},
}
