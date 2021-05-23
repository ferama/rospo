package cmd

import (
	"os/user"
	"path/filepath"

	"github.com/ferama/rospo/pkg/conf"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(grabpubkeyCmd)

	usr, _ := user.Current()
	knownHostFile := filepath.Join(usr.HomeDir, ".ssh", "known_hosts")
	grabpubkeyCmd.PersistentFlags().StringP("known-hosts", "k", knownHostFile, "the known_hosts file absolute path")
}

var grabpubkeyCmd = &cobra.Command{
	Use:   "grabpubkey [host:port]",
	Short: "Grab the host pubkey and put it into the known_hosts file",
	Long:  `Grab the host pubkey and put it into the known_hosts file`,
	Args:  cobra.MinimumNArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		knownHosts, _ := cmd.Flags().GetString("known-hosts")
		sshcConf := &conf.SshClientConf{
			KnownHosts: knownHosts,
			ServerURI:  args[0],
		}
		client := sshc.NewSshConnection(sshcConf)
		client.GrabPubKey()
	},
}
