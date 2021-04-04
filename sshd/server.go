package sshd

import (
	"encoding/binary"
	"fmt"
	"io"
	"io/ioutil"
	"log"
	"net"
	"os"
	"os/exec"
	"sync"
	"syscall"

	"github.com/creack/pty"
	"golang.org/x/crypto/ssh"
)

var (
	DEFAULT_SHELL string = "sh"
)

var (
	hostPrivateKeySigner ssh.Signer
)

type directTCPPayload struct {
	Addr       string // To connect to
	Port       uint32
	OriginAddr string
	OriginPort uint32
}
type tcpIpForwardPayload struct {
	Addr string
	Port uint32
}
type tcpIpForwardPayloadReply struct {
	Port uint32
}
type forwardedTCPPayload struct {
	Addr       string // Is connected to
	Port       uint32
	OriginAddr string
	OriginPort uint32
}

type SshServer struct {
	authorizedKeysMap map[string]bool
	client            *ssh.ServerConn
}

func NewSshServer() *SshServer {
	keyPath := "./id_rsa"
	if os.Getenv("HOST_KEY") != "" {
		keyPath = os.Getenv("HOST_KEY")
	}

	hostPrivateKey, err := ioutil.ReadFile(keyPath)
	if err != nil {
		panic(err)
	}

	// Public key authentication is done by comparing
	// the public key of a received connection
	// with the entries in the authorized_keys file.
	authorizedKeysBytes, err := ioutil.ReadFile("authorized_keys")
	if err != nil {
		log.Fatalf("Failed to load authorized_keys, err: %v", err)
	}
	authorizedKeysMap := map[string]bool{}
	for len(authorizedKeysBytes) > 0 {
		pubKey, _, _, rest, err := ssh.ParseAuthorizedKey(authorizedKeysBytes)
		if err != nil {
			log.Fatal(err)
		}

		authorizedKeysMap[string(pubKey.Marshal())] = true
		authorizedKeysBytes = rest
	}

	hostPrivateKeySigner, err = ssh.ParsePrivateKey(hostPrivateKey)
	if err != nil {
		panic(err)
	}
	ss := &SshServer{
		authorizedKeysMap: authorizedKeysMap,
	}

	return ss
}

func (s *SshServer) keyAuth(conn ssh.ConnMetadata, pubKey ssh.PublicKey) (*ssh.Permissions, error) {
	log.Println(conn.RemoteAddr(), "authenticate with", pubKey.Type())
	if s.authorizedKeysMap[string(pubKey.Marshal())] {
		return &ssh.Permissions{
			// Record the public key used for authentication.
			Extensions: map[string]string{
				"pubkey-fp": ssh.FingerprintSHA256(pubKey),
			},
		}, nil
	}
	return nil, fmt.Errorf("unknown public key for %q", conn.User())
}

func (s *SshServer) Start() {
	config := ssh.ServerConfig{
		PublicKeyCallback: s.keyAuth,
	}
	config.AddHostKey(hostPrivateKeySigner)

	port := "2222"
	if os.Getenv("PORT") != "" {
		port = os.Getenv("PORT")
	}
	socket, err := net.Listen("tcp", ":"+port)
	if err != nil {
		panic(err)
	}
	log.Println("Listening...")
	for {
		conn, err := socket.Accept()
		if err != nil {
			panic(err)
		}

		// From a standard TCP connection to an encrypted SSH connection
		sshConn, chans, reqs, err := ssh.NewServerConn(conn, &config)
		if err != nil {
			// panic(err)
			log.Println(err)
			continue
		}
		s.client = sshConn

		log.Println("Connection from", sshConn.RemoteAddr())
		// Print incoming out-of-band Requests
		go s.handleRequests(reqs)
		// Accept all channels
		go s.handleChannels(chans)
	}
}

// Start assigns a pseudo-terminal tty os.File to c.Stdin, c.Stdout,
// and c.Stderr, calls c.Start, and returns the File of the tty's
// corresponding pty.
func (s *SshServer) ptyRun(c *exec.Cmd, tty *os.File) (err error) {
	defer tty.Close()
	c.Stdout = tty
	c.Stdin = tty
	c.Stderr = tty
	c.SysProcAttr = &syscall.SysProcAttr{
		Setctty: true,
		Setsid:  true,
	}
	return c.Start()
}

func (s *SshServer) handleRequests(reqs <-chan *ssh.Request) {
	for req := range reqs {
		if req.Type == "tcpip-forward" {
			s.handleTcpIpForward(req)
			continue
		}
		log.Printf("recieved out-of-band request: %+v", req)
	}
}

func (s *SshServer) handleTcpIpForward(req *ssh.Request) {
	var payload tcpIpForwardPayload
	if err := ssh.Unmarshal(req.Payload, &payload); err != nil {
		log.Printf("[Unable to unmarshal payload")
		req.Reply(false, []byte{})

		return
	}
	laddr := payload.Addr
	lport := payload.Port

	bind := fmt.Sprintf("[%s]:%d", laddr, lport)
	ln, err := net.Listen("tcp", bind)
	if err != nil {
		log.Printf("Listen failed for %s", bind)
		req.Reply(false, []byte{})
		return
	}
	// Tell client everything is OK
	reply := tcpIpForwardPayloadReply{lport}
	req.Reply(true, ssh.Marshal(&reply))
	// go handleListener(bindinfo, listener)
	go s.handleTcpIpForwardSession(ln, laddr, lport)
}

func (s *SshServer) handleTcpIpForwardSession(listener net.Listener, laddr string, lport uint32) {
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

		// go handleForwardTcpIp(client, bindinfo, lconn)
		go func(lconn net.Conn, laddr string, lport uint32) {
			remotetcpaddr := lconn.RemoteAddr().(*net.TCPAddr)
			raddr := remotetcpaddr.IP.String()
			rport := uint32(remotetcpaddr.Port)
			payload := forwardedTCPPayload{laddr, lport, raddr, uint32(rport)}
			mpayload := ssh.Marshal(&payload)

			c, requests, err := s.client.OpenChannel("forwarded-tcpip", mpayload)
			if err != nil {
				log.Printf("Unable to get channel: %s. Hanging up requesting party!", err)
				lconn.Close()
				return
			}
			go ssh.DiscardRequests(requests)
			// serve(c, lconn, client, *forwardedtimeout)
			// Teardown session
			var once sync.Once
			close := func() {
				c.Close()
				lconn.Close()
				log.Printf("session closed")
			}
			go func() {
				io.Copy(c, lconn)
				once.Do(close)
			}()
			go func() {
				io.Copy(lconn, c)
				once.Do(close)
			}()
		}(lconn, laddr, lport)
	}
}

func (s *SshServer) handleChannelSession(c ssh.NewChannel) {
	channel, requests, err := c.Accept()
	if err != nil {
		log.Printf("could not accept channel (%s)", err)
		return
	}
	// allocate a terminal for this channel
	log.Print("creating pty...")
	// Create new pty
	f, tty, err := pty.Open()
	if err != nil {
		log.Printf("could not start pty (%s)", err)
		return
	}
	var shell string
	shell = os.Getenv("SHELL")
	if shell == "" {
		shell = DEFAULT_SHELL
	}
	for req := range requests {
		// log.Printf("%v %s", req.Payload, req.Payload)
		ok := false
		switch req.Type {
		case "exec":
			ok = true
			command := string(req.Payload[4 : req.Payload[3]+4])
			cmd := exec.Command(shell, []string{"-c", command}...)

			cmd.Stdout = channel
			cmd.Stderr = channel
			cmd.Stdin = channel

			err := cmd.Start()
			if err != nil {
				log.Printf("could not start command (%s)", err)
				continue
			}

			// teardown session
			go func() {
				_, err := cmd.Process.Wait()
				if err != nil {
					log.Printf("failed to exit bash (%s)", err)
				}
				channel.Close()
				log.Printf("session closed")
			}()
		case "shell":
			cmd := exec.Command(shell)
			cmd.Env = []string{"TERM=xterm"}
			err := s.ptyRun(cmd, tty)
			if err != nil {
				log.Printf("%s", err)
			}

			// Teardown session
			var once sync.Once
			close := func() {
				channel.Close()
				log.Printf("session closed")
			}

			// Pipe session to bash and visa-versa
			go func() {
				io.Copy(channel, f)
				once.Do(close)
			}()

			go func() {
				io.Copy(f, channel)
				once.Do(close)
			}()

			// We don't accept any commands (Payload),
			// only the default shell.
			if len(req.Payload) == 0 {
				ok = true
			}
		case "pty-req":
			// Responding 'ok' here will let the client
			// know we have a pty ready for input
			ok = true
			// Parse body...
			termLen := req.Payload[3]
			termEnv := string(req.Payload[4 : termLen+4])
			w, h := s.parseDims(req.Payload[termLen+4:])
			SetWinsize(f.Fd(), w, h)
			log.Printf("pty-req '%s'", termEnv)
		case "window-change":
			w, h := s.parseDims(req.Payload)
			SetWinsize(f.Fd(), w, h)
			continue //no response
		}

		if !ok {
			log.Printf("declining %s request...", req.Type)
		}

		req.Reply(ok, nil)
	}
}

func (s *SshServer) handleChannelDirect(c ssh.NewChannel) {
	var payload directTCPPayload
	if err := ssh.Unmarshal(c.ExtraData(), &payload); err != nil {
		log.Printf("Could not unmarshal extra data: %s\n", err)

		c.Reject(ssh.Prohibited, "Bad payload")
		return
	}
	connection, requests, err := c.Accept()
	if err != nil {
		log.Printf("Could not accept channel (%s)\n", err)
		return
	}
	go ssh.DiscardRequests(requests)
	addr := fmt.Sprintf("[%s]:%d", payload.Addr, payload.Port)

	rconn, err := net.Dial("tcp", addr)
	if err != nil {
		log.Printf("Could not dial remote (%s)", err)
		connection.Close()
		return
	}
	// Teardown session
	var once sync.Once
	close := func() {
		connection.Close()
		rconn.Close()
		log.Printf("session closed")
	}
	go func() {
		io.Copy(connection, rconn)
		once.Do(close)
	}()
	go func() {
		io.Copy(rconn, connection)
		once.Do(close)
	}()
}

func (s *SshServer) handleChannels(chans <-chan ssh.NewChannel) {
	// Service the incoming Channel channel.
	for newChannel := range chans {
		t := newChannel.ChannelType()
		if t == "session" {
			go s.handleChannelSession(newChannel)
			continue
		}
		if t == "direct-tcpip" {
			go s.handleChannelDirect(newChannel)
			continue
		}
		newChannel.Reject(ssh.UnknownChannelType, fmt.Sprintf("unknown channel type: %s", t))
	}
}

// parseDims extracts two uint32s from the provided buffer.
func (s *SshServer) parseDims(b []byte) (uint32, uint32) {
	w := binary.BigEndian.Uint32(b)
	h := binary.BigEndian.Uint32(b[4:])
	return w, h
}
