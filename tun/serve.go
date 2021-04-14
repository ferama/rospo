package tun

import (
	"io"
	"net"
	"sync"
)

func serveClient(client net.Conn, remote net.Conn) {
	var once sync.Once
	close := func() {
		client.Close()
		remote.Close()
	}

	// Start remote -> local data transfer
	go func() {
		io.Copy(client, remote)
		once.Do(close)

	}()

	// Start local -> remote data transfer
	go func() {
		io.Copy(remote, client)
		once.Do(close)
	}()
}
