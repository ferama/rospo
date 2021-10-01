package sshd

import (
	"net"

	"github.com/ferama/rospo/pkg/utils"
	"golang.org/x/crypto/ssh"
)

func handleTcpIpForwardSession(client *ssh.ServerConn, listener net.Listener, laddr string, lport uint32) {
	for {
		lconn, err := listener.Accept()
		if err != nil {
			neterr := err.(net.Error)
			if neterr.Timeout() {
				log.Printf("Accept failed with timeout: %s", err)
				continue
			}
			if neterr.Temporary() {
				log.Printf("Accept failed with temporary: %s", err)
				continue
			}

			break
		}
		log.Printf("started forward session: %s", lconn.LocalAddr())

		go func(lconn net.Conn, laddr string, lport uint32) {
			remotetcpaddr := lconn.RemoteAddr().(*net.TCPAddr)
			raddr := remotetcpaddr.IP.String()
			rport := uint32(remotetcpaddr.Port)

			var payload = struct {
				Addr       string // Is connected to
				Port       uint32
				OriginAddr string
				OriginPort uint32
			}{
				laddr, lport, raddr, uint32(rport),
			}

			mpayload := ssh.Marshal(payload)

			c, requests, err := client.OpenChannel("forwarded-tcpip", mpayload)
			if err != nil {
				log.Printf("Unable to get channel: %s. Hanging up requesting party!", err)
				lconn.Close()
				return
			}
			go ssh.DiscardRequests(requests)
			utils.CopyConn(c, lconn)
			log.Printf("ended forward session: %s", lconn.LocalAddr())
		}(lconn, laddr, lport)
	}
}
