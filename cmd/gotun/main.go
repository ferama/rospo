package main

import (
	"gotun/tun"
	"gotun/utils"
	"log"
	"time"
)

func main() {
	flags := utils.GetFlags()

	username := flags.Username
	identity := flags.Identity
	localEndpoint := tun.NewEndpoint(*flags.LocalEndpoint)
	serverEndpoint := tun.NewEndpoint(*flags.ServerEndpoint)
	remoteEndpoint := tun.NewEndpoint(*flags.RemoteEndpoint)

	for {
		log.Println("connecting...")
		if *flags.Forward {
			tun.ForwardTunnel(
				*username,
				*identity,
				serverEndpoint,
				remoteEndpoint,
				localEndpoint)
		} else {
			tun.ReverseTunnel(
				*username,
				*identity,
				serverEndpoint,
				remoteEndpoint,
				localEndpoint)
		}

		time.Sleep(3 * time.Second)
	}
}
