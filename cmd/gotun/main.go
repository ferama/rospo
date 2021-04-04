package main

import (
	"fmt"
	"gotun/conn"
	"gotun/utils"
	"log"
	"net"
	"time"
)

func main() {
	flags := utils.GetFlags()

	localEndpoint := conn.NewEndpoint(*flags.LocalEndpoint)
	serverEndpoint := conn.NewEndpoint(*flags.ServerEndpoint)
	remoteEndpoint := conn.NewEndpoint(*flags.RemoteEndpoint)

	for {
		log.Println("connecting...")
		serverConn, listener := conn.Connect(serverEndpoint, remoteEndpoint)
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
				conn.HandleClient(client, local)
			}
			serverConn.Close()
			listener.Close()
		}
		time.Sleep(3 * time.Second)
	}
}
