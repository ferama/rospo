package cmd

import (
	"log"
	"os"
	"os/signal"

	"github.com/ferama/rospo/pkg/conf"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/ferama/rospo/pkg/sshd"
	"github.com/ferama/rospo/pkg/tun"
	"github.com/ferama/rospo/pkg/web"
	rootapi "github.com/ferama/rospo/pkg/web/api/root"
	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(runCmd)
}

var runCmd = &cobra.Command{
	Use:   "run config_file_path.yaml",
	Short: "Run rospo using a config file.",
	Long:  "Run rospo using a config file.",
	Args:  cobra.MinimumNArgs(1),
	ValidArgsFunction: func(cmd *cobra.Command, args []string, toComplete string) ([]string, cobra.ShellCompDirective) {
		if len(args) != 0 {
			return nil, cobra.ShellCompDirectiveNoFileComp
		}
		return []string{"yaml"}, cobra.ShellCompDirectiveFilterFileExt
	},
	Run: func(cmd *cobra.Command, args []string) {
		conf, err := conf.LoadConfig(args[0])
		if err != nil {
			log.Fatalln(err)
		}

		var sshConn *sshc.SshConnection
		if conf.Tunnel != nil || conf.Web != nil || conf.SocksProxy != nil {
			if conf.SshClient == nil {
				log.Fatalln("you need to configure sshclient section to support tunnels or webui")
			}
			sshConn = sshc.NewSshConnection(conf.SshClient)
			go sshConn.Start()
		}

		if conf.SshD != nil {
			sshServer := sshd.NewSshServer(conf.SshD)
			go sshServer.Start()
		}

		if conf.Tunnel != nil && len(conf.Tunnel) > 0 {
			for _, c := range conf.Tunnel {
				if c.SshClientConf != nil {
					conn := sshc.NewSshConnection(c.SshClientConf)
					go conn.Start()
					go tun.NewTunnel(conn, c, false).Start()
				} else {
					go tun.NewTunnel(sshConn, c, false).Start()
				}
			}
		}

		if conf.Web != nil {
			dev := false
			if Version == "development" {
				dev = true
			}
			jh := []string{}
			info := &rootapi.Info{}
			if conf.SshClient != nil {
				for _, h := range conf.SshClient.JumpHosts {
					jh = append(jh, h.URI)
				}
				info = &rootapi.Info{
					SshClientURI: conf.SshClient.ServerURI,
					JumpHosts:    jh,
				}
			}

			go web.StartServer(dev, sshConn, conf.Web, info)
		}

		if conf.SocksProxy != nil {
			var sockProxy *sshc.SocksProxy
			if conf.SocksProxy.SshClientConf == nil {
				sockProxy = sshc.NewSockProxy(sshConn)
			} else {
				proxySshConn := sshc.NewSshConnection(conf.SocksProxy.SshClientConf)
				go proxySshConn.Start()
				sockProxy = sshc.NewSockProxy(proxySshConn)
			}
			go func() {
				err := sockProxy.Start(conf.SocksProxy.ListenAddress)
				if err != nil {
					log.Fatal(err)
				}
			}()
		}

		c := make(chan os.Signal, 1)
		signal.Notify(c, os.Interrupt)
		<-c
	},
}
