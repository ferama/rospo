package cmd

import (
	"log"

	"github.com/ferama/rospo/cmd/cmnflags"
	"github.com/ferama/rospo/pkg/autocomplete"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(socksProxyCmd)
	// sshc options
	cmnflags.AddSshClientFlags(socksProxyCmd.Flags())

	socksProxyCmd.Flags().StringP("listen-address", "l", "127.0.0.1:1080", "the socks proxy listener address")
}

var socksProxyCmd = &cobra.Command{
	Use:   "socks-proxy [user@]host[:port]",
	Short: "Starts a SOCKS proxy",
	Long: `Starts a SOCKS proxy

Both version 4 and 5 are supported.
You need to configure your browser to use the SOCKS proxy.
On windows you should put somthing like "socks=localhost" into the address field into
the proxy configuration form.
	
	`,
	Example: `
  # start a socks proxy on 127.0.0.1:1080
  $ rospo proxy sshhost:sshport

	`,
	Args:              cobra.MinimumNArgs(1),
	ValidArgsFunction: autocomplete.Host(),
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
