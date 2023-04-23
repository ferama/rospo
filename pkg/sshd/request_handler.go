package sshd

import (
	"fmt"
	"net"
	"strconv"
	"strings"
	"sync"
	"time"

	"golang.org/x/crypto/ssh"
)

type requestHandler struct {
	sshConn *ssh.ServerConn

	reqs <-chan *ssh.Request

	forwards   map[string]net.Listener
	forwardsMu sync.Mutex

	forwardsKeepAliveInterval time.Duration
}

func newRequestHandler(sshConn *ssh.ServerConn, reqs <-chan *ssh.Request) *requestHandler {
	return &requestHandler{
		sshConn:                   sshConn,
		reqs:                      reqs,
		forwards:                  make(map[string]net.Listener),
		forwardsKeepAliveInterval: 5 * time.Second,
	}
}

func (r *requestHandler) tcpipForwardHandler(req *ssh.Request) {
	var payload = struct {
		Addr string
		Port uint32
	}{}
	if err := ssh.Unmarshal(req.Payload, &payload); err != nil {
		log.Printf("Unable to unmarshal payload")
		req.Reply(false, []byte{})
		return
	}
	laddr := payload.Addr
	lport := payload.Port
	addr := fmt.Sprintf("[%s]:%d", laddr, lport)

	ln, err := net.Listen("tcp", addr)
	if err != nil {
		log.Printf("listen failed for %s %s", addr, err)
		req.Reply(false, []byte{})
		return
	}

	// if a random port was requested, extract it from the listener
	// and use that as lport var. The lport value will be sent as reply
	// to the client
	if lport == 0 {
		_, port, err := net.SplitHostPort(ln.Addr().String())
		if err != nil {
			panic(err)
		}
		u64, err := strconv.ParseUint(port, 10, 32)
		if err != nil {
			panic(err)
		}
		lport = uint32(u64)
		// fix the addr value too
		addr = fmt.Sprintf("[%s]:%d", laddr, lport)
	}
	log.Printf("tcpip-forward listening for %s", addr)
	var replyPayload = struct{ Port uint32 }{lport}
	// Tell client everything is OK
	req.Reply(true, ssh.Marshal(replyPayload))
	// go handleTcpIpForwardSession(r.sshConn, ln, laddr, lport)
	forwardSessionHandler := newSessionHandler(r.sshConn, ln, laddr, lport)
	go forwardSessionHandler.handleSession()

	go r.checkAlive(r.sshConn, ln, addr)

	r.forwardsMu.Lock()
	r.forwards[addr] = ln
	r.forwardsMu.Unlock()
}

func (r *requestHandler) cancelTcpIpForwardHandler(req *ssh.Request) {
	var payload = struct {
		Addr string
		Port uint32
	}{}
	if err := ssh.Unmarshal(req.Payload, &payload); err != nil {
		log.Printf("Unable to unmarshal payload")
		req.Reply(false, []byte{})
		return
	}
	// TODO: what happens here if the original port was 0 (random port)?
	laddr := payload.Addr
	lport := payload.Port
	addr := fmt.Sprintf("[%s]:%d", laddr, lport)
	r.forwardsMu.Lock()
	ln, ok := r.forwards[addr]
	r.forwardsMu.Unlock()
	if ok {
		ln.Close()
	}
	req.Reply(true, nil)
}

func (r *requestHandler) handleRequests() {
	for req := range r.reqs {
		switch req.Type {
		case "tcpip-forward":
			r.tcpipForwardHandler(req)

		case "cancel-tcpip-forward":
			r.cancelTcpIpForwardHandler(req)
		default:
			if strings.Contains(req.Type, "keepalive") {
				req.Reply(true, nil)
				continue
			}
			log.Printf("received out-of-band request: %+v", req)
		}
	}
}

func (r *requestHandler) checkAlive(sshConn *ssh.ServerConn, ln net.Listener, addr string) {
	ticker := time.NewTicker(r.forwardsKeepAliveInterval)

	log.Println("starting check for forward availability")
	for {
		<-ticker.C
		_, _, err := sshConn.SendRequest("checkalive@rospo", true, nil)
		if err != nil {
			log.Printf("forward endpoint not available anymore. Closing socket %s", ln.Addr())
			ln.Close()
			r.forwardsMu.Lock()
			delete(r.forwards, addr)
			r.forwardsMu.Unlock()
			return
		}
	}
}
