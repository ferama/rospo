package cmd

import (
	"os/user"
	"path/filepath"

	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(tunCmd)

	usr, _ := user.Current()
	defaultIdentity := filepath.Join(usr.HomeDir, ".ssh", "id_rsa")

	tunCmd.PersistentFlags().BoolP("insecure", "i", false, "disable known_hosts key server verification")
	tunCmd.PersistentFlags().StringP("jump-host", "j", "", "optional jump host conf")
	tunCmd.PersistentFlags().StringP("user-identity", "k", defaultIdentity, "the ssh identity (private) key absolute path")
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
