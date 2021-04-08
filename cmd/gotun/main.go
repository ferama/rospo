package main

import (
	"gotun/sshd"
	"gotun/tun"
	"gotun/utils"
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

		tun.NewTunnel(
			*username,
			*userIdentity,
			serverEndpoint,
			remoteEndpoint,
			localEndpoint,
			*flags.Forward,
		).Start()
	}
}
