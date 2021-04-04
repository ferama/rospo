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

type SshServer struct {
	authorizedKeysMap map[string]bool
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
		log.Printf("recieved out-of-band request: %+v", req)
	}
}

func (s *SshServer) handleChannelsRequests(channel ssh.Channel, requests <-chan *ssh.Request) {
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

func (s *SshServer) handleChannels(chans <-chan ssh.NewChannel) {
	// Service the incoming Channel channel.
	for newChannel := range chans {
		// Channels have a type, depending on the application level
		// protocol intended. In the case of a shell, the type is
		// "session" and ServerShell may be used to present a simple
		// terminal interface.
		if t := newChannel.ChannelType(); t != "session" {
			newChannel.Reject(ssh.UnknownChannelType, fmt.Sprintf("unknown channel type: %s", t))
			continue
		}
		channel, requests, err := newChannel.Accept()
		if err != nil {
			log.Printf("could not accept channel (%s)", err)
			continue
		}
		s.handleChannelsRequests(channel, requests)
	}
}

// parseDims extracts two uint32s from the provided buffer.
func (s *SshServer) parseDims(b []byte) (uint32, uint32) {
	w := binary.BigEndian.Uint32(b)
	h := binary.BigEndian.Uint32(b[4:])
	return w, h
}
