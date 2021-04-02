package main

import (
	"fmt"
	"log"
	"net"
	"time"
)

func main() {
	flags := GetFlags()

	localEndpoint := NewEndpoint(*flags.LocalEndpoint)
	serverEndpoint := NewEndpoint(*flags.ServerEndpoint)
	remoteEndpoint := NewEndpoint(*flags.RemoteEndpoint)

	for {
		log.Println("connecting...")
		serverConn, listener := Connect(serverEndpoint, remoteEndpoint)
		for {
			// Open a (local) connection to localEndpoint whose content will be forwarded so serverEndpoint
			local, err := net.Dial("tcp", localEndpoint.String())
			if err != nil {
				log.Fatalln(fmt.Printf("Dial INTO local service error: %s", err))
			}

			client, err := listener.Accept()
			if err != nil {
				log.Println("disconnected")
				break
			}
			HandleClient(client, local)
		}
		serverConn.Close()
		listener.Close()
		time.Sleep(1 * time.Second)
	}
}
