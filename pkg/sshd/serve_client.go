package sshd

import (
	"io"
	"log"
	"net"
	"sync"

	"golang.org/x/crypto/ssh"
)

func serveClient(cssh ssh.Channel, conn net.Conn) {
	var once sync.Once
	close := func() {
		cssh.Close()
		conn.Close()
		log.Printf("[SSHD] direct-tcpip session closed")
	}
	go func() {
		io.Copy(cssh, conn)
		once.Do(close)
	}()
	go func() {
		io.Copy(conn, cssh)
		once.Do(close)
	}()
}
