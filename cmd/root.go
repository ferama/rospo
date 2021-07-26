package cmd

import (
	"log"

	"github.com/ferama/rospo/pkg/conf"
	"github.com/ferama/rospo/pkg/pipe"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/ferama/rospo/pkg/sshd"
	"github.com/ferama/rospo/pkg/tun"
	"github.com/ferama/rospo/pkg/web"
	"github.com/spf13/cobra"
)

var Version = "development"

var rootCmd = &cobra.Command{
	Use:     "rospo config_file_path.yaml",
	Long:    "The tool to create relieable ssh tunnels.",
	Version: Version,
	Args:    cobra.MinimumNArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		conf, err := conf.LoadConfig(args[0])
		if err != nil {
			log.Fatalln(err)
		}

		var sshConn *sshc.SshConnection
		if conf.Tunnel != nil || conf.Web != nil {
			if conf.SshClient == nil {
				log.Fatalln("You need to configure sshclient section to support tunnels")
			}
			sshConn = sshc.NewSshConnection(conf.SshClient)
			go sshConn.Start()
		}

		if conf.Web != nil {
			dev := false
			if Version == "development" {
				dev = true
			}
			go web.StartServer(dev, sshConn, conf.Web)
		}

		if conf.Pipe != nil {
			for _, f := range conf.Pipe {
				go pipe.NewPipe(f).Start()
			}
		}

		if conf.SshD != nil {
			if conf.Tunnel != nil {
				go sshd.NewSshServer(conf.SshD).Start()
			} else {
				sshd.NewSshServer(conf.SshD).Start()
			}
		}

		if conf.Tunnel != nil && len(conf.Tunnel) > 0 {
			for idx, c := range conf.Tunnel {
				if idx == len(conf.Tunnel)-1 {
					tun.NewTunnel(sshConn, c).Start()
				} else {
					go tun.NewTunnel(sshConn, c).Start()
				}
			}
		}
	},
}

// Execute executes the root command
func Execute() error {
	return rootCmd.Execute()
}
