package cmd

import (
	"log"

	"github.com/ferama/rospo/cmd/cmnflags"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(proxyCmd)
	// sshc options
	cmnflags.AddSshClientFlags(proxyCmd.Flags())

	proxyCmd.Flags().StringP("listen-address", "l", "127.0.0.1:1080", "the socks proxy listener address")
}

var proxyCmd = &cobra.Command{
	Use:   "proxy [user@]host[:port]",
	Short: "Starts a SOCKS5 proxy",
	Long:  "Starts a SOCKS5 proxy",
	Args:  cobra.MinimumNArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		sshcConf := cmnflags.GetSshClientConf(cmd, args[0])
		conn := sshc.NewSshConnection(sshcConf)
		go conn.Start()

		listenAddress, _ := cmd.Flags().GetString("listen-address")

		sockProxy := sshc.NewSocksProxy(conn)
		err := sockProxy.Start(listenAddress)
		if err != nil {
			log.Fatalln(err)
		}
	},
}
