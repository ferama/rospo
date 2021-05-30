package sshd

import (
	"fmt"
	"log"
	"net"

	"github.com/ferama/rospo/pkg/utils"
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

	utils.CopyConn(connection, rconn)
}
