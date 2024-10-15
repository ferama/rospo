package cmd

import (
	"log"

	"github.com/ferama/rospo/cmd/cmnflags"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(dnsProxyCmd)
	// sshc options
	cmnflags.AddSshClientFlags(dnsProxyCmd.Flags())

	dnsProxyCmd.Flags().StringP("listen-address", "l", ":53", "the dns proxy listener address")
	dnsProxyCmd.Flags().StringP("remote-dns-server", "d", sshc.DEFAULT_DNS_SERVER, "the dns address to reach through sshc")
}

var dnsProxyCmd = &cobra.Command{
	Use:   "dns-proxy [user@]host[:port]",
	Short: "Starts a dns proxy",
	Long: `Starts a local dns server that sends its request through the ssh tunnel
to the configured DNS server.
	`,
	Args: cobra.MinimumNArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		sshcConf := cmnflags.GetSshClientConf(cmd, args[0])
		conn := sshc.NewSshConnection(sshcConf)
		go conn.Start()

		listenAddress, _ := cmd.Flags().GetString("listen-address")
		remoteDnsServer, _ := cmd.Flags().GetString("remote-dns-server")

		conf := &sshc.DnsProxyConf{
			ListenAddress:    listenAddress,
			RemoteDnsAddress: &remoteDnsServer,
		}
		proxy := sshc.NewDnsProxy(conn, conf)
		err := proxy.Start()
		if err != nil {
			log.Fatalln(err)
		}
	},
}
