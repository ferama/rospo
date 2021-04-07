package sshd

import (
	"fmt"
	"io"
	"log"
	"net"
	"sync"

	"golang.org/x/crypto/ssh"
)

func handleChannelDirect(c ssh.NewChannel) {
	var payload = struct {
		Addr       string
		Port       uint32
		OriginAddr string
		OriginPort uint32
	}{}

	if err := ssh.Unmarshal(c.ExtraData(), &payload); err != nil {
		log.Printf("[SSHD] Could not unmarshal extra data: %s\n", err)

		c.Reject(ssh.Prohibited, "Bad payload")
		return
	}
	connection, requests, err := c.Accept()
	if err != nil {
		log.Printf("[SSHD] Could not accept channel (%s)\n", err)
		return
	}
	go ssh.DiscardRequests(requests)
	addr := fmt.Sprintf("[%s]:%d", payload.Addr, payload.Port)

	rconn, err := net.Dial("tcp", addr)
	if err != nil {
		log.Printf("[SSHD] Could not dial remote (%s)", err)
		connection.Close()
		return
	}

	directServe(connection, rconn)
}

func directServe(cssh ssh.Channel, conn net.Conn) {
	var once sync.Once
	close := func() {
		cssh.Close()
		conn.Close()
		log.Printf("[SSHD] direct session closed")
	}
	go func() {
		_, err := io.Copy(cssh, conn)
		if err != nil {
			log.Println(fmt.Sprintf("[SSHD] direct - error while copy: %s", err))
		}
		once.Do(close)
	}()
	go func() {
		_, err := io.Copy(conn, cssh)
		if err != nil {
			log.Println(fmt.Sprintf("[SSHD] direct - error while copy: %s", err))
		}
		once.Do(close)
	}()
}
