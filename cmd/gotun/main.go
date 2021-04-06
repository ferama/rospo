package main

import (
	"gotun/sshd"
	"gotun/tun"
	"gotun/utils"
	"time"
)

func main() {
	flags := utils.GetFlags()

	if !*flags.DisableSshd {
		s := sshd.NewSshServer(
			flags.ServerIdentity,
			flags.ServerAuthorizedKeys,
			flags.SshdPort,
		)
		if !*flags.DisableTun {
			go s.Start()
		} else {
			s.Start()
		}
	}

	if !*flags.DisableTun {

		username := flags.Username
		userIdentity := flags.UserIdentity
		localEndpoint := tun.NewEndpoint(*flags.LocalEndpoint)
		serverEndpoint := tun.NewEndpoint(*flags.ServerEndpoint)
		remoteEndpoint := tun.NewEndpoint(*flags.RemoteEndpoint)

		for {
			if *flags.Forward {
				tun.ForwardTunnel(
					*username,
					*userIdentity,
					serverEndpoint,
					remoteEndpoint,
					localEndpoint)
			} else {
				tun.ReverseTunnel(
					*username,
					*userIdentity,
					serverEndpoint,
					remoteEndpoint,
					localEndpoint)
			}

			time.Sleep(3 * time.Second)
		}
	}
}
