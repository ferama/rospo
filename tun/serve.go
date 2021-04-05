package tun

import (
	"fmt"
	"io"
	"log"
	"net"
	"sync"
)

func serveClient(client net.Conn, remote net.Conn) {
	var once sync.Once
	close := func() {
		client.Close()
		// log.Printf("session closed")
	}

	// Start remote -> local data transfer
	go func() {
		_, err := io.Copy(client, remote)
		if err != nil {
			log.Println(fmt.Sprintf("[TUN] error while copy remote->local: %s", err))
		}
		once.Do(close)

	}()

	// Start local -> remote data transfer
	go func() {
		_, err := io.Copy(remote, client)
		if err != nil {
			log.Println(fmt.Sprintf("[TUN] error while copy local->remote: %s", err))
		}
		once.Do(close)

	}()
}
