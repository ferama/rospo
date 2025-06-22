package sshd

import (
	"net"

	"github.com/ferama/rospo/pkg/rio"
	"golang.org/x/crypto/ssh"
)

type sessionHandler struct {
	sshConn      *ssh.ServerConn
	listener     net.Listener
	listenerAddr string
	listenerPort uint32
}

func newSessionHandler(sshConn *ssh.ServerConn,
	ln net.Listener,
	laddr string,
	lport uint32) *sessionHandler {

	return &sessionHandler{
		sshConn:      sshConn,
		listener:     ln,
		listenerAddr: laddr,
		listenerPort: lport,
	}
}

func (s *sessionHandler) handleClient(client net.Conn) {
	log.Printf("start forward session: %s", client.LocalAddr())

	remotetcpaddr := client.RemoteAddr().(*net.TCPAddr)
	raddr := remotetcpaddr.IP.String()
	rport := uint32(remotetcpaddr.Port)

	var payload = struct {
		Addr       string // Is connected to
		Port       uint32
		OriginAddr string
		OriginPort uint32
	}{
		s.listenerAddr, s.listenerPort, raddr, uint32(rport),
	}

	mpayload := ssh.Marshal(payload)

	c, requests, err := s.sshConn.OpenChannel("forwarded-tcpip", mpayload)
	if err != nil {
		log.Printf("Unable to get channel: %s. Hanging up requesting party!", err)
		client.Close()
		return
	}
	go ssh.DiscardRequests(requests)
	rio.CopyConn(c, client)
	log.Printf("end forward session: %s", client.LocalAddr())
}

func (s *sessionHandler) handleSession() {
	for {
		client, err := s.listener.Accept()
		if err != nil {
			neterr := err.(net.Error)
			if neterr.Timeout() {
				log.Printf("Accept failed with timeout: %s", err)
				continue
			}
			break
		}
		go s.handleClient(client)
	}
}
