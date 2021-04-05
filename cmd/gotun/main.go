package main

import (
	"fmt"
	"gotun/tun"
	"gotun/utils"
	"log"
	"net"
	"time"
)

func main() {
	flags := utils.GetFlags()

	localEndpoint := tun.NewEndpoint(*flags.LocalEndpoint)
	serverEndpoint := tun.NewEndpoint(*flags.ServerEndpoint)
	remoteEndpoint := tun.NewEndpoint(*flags.RemoteEndpoint)

	for {
		log.Println("connecting...")
		serverConn, listener := tun.Connect(serverEndpoint, remoteEndpoint)
		if serverConn != nil && listener != nil {
			for {
				// Open a (local) connection to localEndpoint whose content will be forwarded so serverEndpoint
				local, err := net.Dial("tcp", localEndpoint.String())
				if err != nil {
					log.Println(fmt.Printf("Dial INTO local service error. %s\n", err))
					break
				}

				client, err := listener.Accept()
				if err != nil {
					log.Println("disconnected")
					break
				}
				tun.HandleClient(client, local)
			}
			serverConn.Close()
			listener.Close()
		}
		time.Sleep(3 * time.Second)
	}
}
