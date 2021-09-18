package cmd

import (
	"github.com/ferama/rospo/cmd/cmnflags"
	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(tunCmd)

	cmnflags.AddSshClientFlags(tunCmd.PersistentFlags())

	tunCmd.PersistentFlags().StringP("local", "l", "127.0.0.1:2222", "the local tunnel endpoint")
	tunCmd.PersistentFlags().StringP("remote", "r", "127.0.0.1:2222", "the remote tunnel endpoint")
}

var tunCmd = &cobra.Command{
	Use:   "tun",
	Short: "Creates a reliable ssh tunnel",
	Long:  `Creates a reliable ssh tunnel`,
	Args:  cobra.MinimumNArgs(1),
	Run:   func(cmd *cobra.Command, args []string) {},
}
