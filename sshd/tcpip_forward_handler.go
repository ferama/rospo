package sshd

import (
	"fmt"
	"io"
	"log"
	"net"
	"sync"

	"golang.org/x/crypto/ssh"
)

func handleTcpIpForward(req *ssh.Request, client *ssh.ServerConn) net.Listener {
	var payload = struct {
		Addr string
		Port uint32
	}{}
	if err := ssh.Unmarshal(req.Payload, &payload); err != nil {
		log.Printf("[SSHD] Unable to unmarshal payload")
		req.Reply(false, []byte{})
		return nil
	}
	laddr := payload.Addr
	lport := payload.Port

	bind := fmt.Sprintf("[%s]:%d", laddr, lport)
	ln, err := net.Listen("tcp", bind)
	if err != nil {
		log.Printf("[SSHD] Listen failed for %s", bind)
		req.Reply(false, []byte{})
		return nil
	}
	var replyPayload = struct{ Port uint32 }{lport}
	// Tell client everything is OK
	req.Reply(true, ssh.Marshal(replyPayload))
	// go handleListener(bindinfo, listener)
	go handleTcpIpForwardSession(client, ln, laddr, lport)

	return ln
}

func handleTcpIpForwardSession(client *ssh.ServerConn, listener net.Listener, laddr string, lport uint32) {
	for {
		lconn, err := listener.Accept()
		if err != nil {
			neterr := err.(net.Error)
			if neterr.Timeout() {
				log.Printf("[SSHD] Accept failed with timeout: %s", err)
				continue
			}
			if neterr.Temporary() {
				log.Printf("[SSHD] Accept failed with temporary: %s", err)
				continue
			}

			break
		}

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
				log.Printf("[SSHD] Unable to get channel: %s. Hanging up requesting party!", err)
				lconn.Close()
				return
			}
			go ssh.DiscardRequests(requests)
			forwardServe(c, lconn)
		}(lconn, laddr, lport)
	}
}

func forwardServe(cssh ssh.Channel, conn net.Conn) {
	var once sync.Once
	close := func() {
		cssh.Close()
		conn.Close()
		log.Printf("[SSHD] forward session closed")
	}
	go func() {
		_, err := io.Copy(cssh, conn)
		if err != nil {
			log.Println(fmt.Sprintf("[SSHD] forward - error while copy: %s", err))
		}
		once.Do(close)
	}()
	go func() {
		_, err := io.Copy(conn, cssh)
		if err != nil {
			log.Println(fmt.Sprintf("[SSHD] forward - error while copy: %s", err))
		}
		once.Do(close)
	}()
}
