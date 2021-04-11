package sshd

import (
	"fmt"
	"io/ioutil"
	"log"
	"net"
	"rospo/utils"
	"strings"

	"golang.org/x/crypto/ssh"
)

var (
	DEFAULT_SHELL string = "sh"
)

type SshServer struct {
	client            *ssh.ServerConn
	hostPrivateKey    ssh.Signer
	authorizedKeyFile *string
	tcpPort           *string

	// tcpIpForwardListener net.Listener
	forwards map[string]net.Listener
}

func NewSshServer(identity *string, authorizedKeys *string, tcpPort *string) *SshServer {
	hostPrivateKey, err := ioutil.ReadFile(*identity)
	if err != nil {
		log.Println("[SSHD] server identity do not exists. Generating one...")
		utils.GeneratePrivateKey(identity)
		hostPrivateKey, err = ioutil.ReadFile(*identity)
		if err != nil {
			panic(err)
		}
	}

	hostPrivateKeySigner, err := ssh.ParsePrivateKey(hostPrivateKey)
	if err != nil {
		panic(err)
	}

	ss := &SshServer{
		authorizedKeyFile: authorizedKeys,
		hostPrivateKey:    hostPrivateKeySigner,
		tcpPort:           tcpPort,
		forwards:          make(map[string]net.Listener),
	}

	// run here, to make sure I have a valid authorized keys
	// file on start
	ss.loadAuthorizedKeys()

	return ss
}

func (s *SshServer) loadAuthorizedKeys() map[string]bool {
	// Public key authentication is done by comparing
	// the public key of a received connection
	// with the entries in the authorized_keys file.
	authorizedKeysBytes, err := ioutil.ReadFile(*s.authorizedKeyFile)
	if err != nil {
		log.Fatalf(`Failed to load authorized_keys, err: %v

	Please create ./authorized_keys file and fill in with 
	your authorized users public keys

`, err)
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
	return authorizedKeysMap
}

func (s *SshServer) keyAuth(conn ssh.ConnMetadata, pubKey ssh.PublicKey) (*ssh.Permissions, error) {
	log.Println("[SSHD] ", conn.RemoteAddr(), "authenticate with", pubKey.Type())

	authorizedKeysMap := s.loadAuthorizedKeys()

	if authorizedKeysMap[string(pubKey.Marshal())] {
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
		// one try only. I'm supporting public key auth only.
		// If it fails, there is nothing more to try
		MaxAuthTries:      1,
		PublicKeyCallback: s.keyAuth,
		AuthLogCallback: func(conn ssh.ConnMetadata, method string, err error) {
			if err != nil {
				log.Printf("[SSHD] auth error: %s", err)
			}
		},
	}
	config.AddHostKey(s.hostPrivateKey)

	socket, err := net.Listen("tcp", ":"+*s.tcpPort)
	if err != nil {
		panic(err)
	}
	log.Printf("[SSHD] Listening on port %s\n", *s.tcpPort)
	for {
		conn, err := socket.Accept()
		if err != nil {
			panic(err)
		}

		go func() {
			// From a standard TCP connection to an encrypted SSH connection
			sshConn, chans, reqs, err := ssh.NewServerConn(conn, &config)
			if err != nil {
				log.Printf("[SSHD] %s", err)
				return
			}
			log.Printf("[SSHD] logged in with key %s", sshConn.Permissions.Extensions["pubkey-fp"])

			s.client = sshConn

			log.Println("[SSHD] Connection from", sshConn.RemoteAddr())
			// Print incoming out-of-band Requests
			go s.handleRequests(reqs)
			// Accept all channels
			go s.handleChannels(chans)
		}()
	}
}

func (s *SshServer) handleRequests(reqs <-chan *ssh.Request) {
	for req := range reqs {
		switch req.Type {
		case "tcpip-forward":
			var payload = struct {
				Addr string
				Port uint32
			}{}
			if err := ssh.Unmarshal(req.Payload, &payload); err != nil {
				log.Printf("[SSHD] Unable to unmarshal payload")
				req.Reply(false, []byte{})
				continue
			}
			laddr := payload.Addr
			lport := payload.Port
			addr := fmt.Sprintf("[%s]:%d", laddr, lport)
			ln, ok := s.forwards[addr]
			if ok {
				ln.Close()
			}

			ln, err := net.Listen("tcp", addr)
			if err != nil {
				log.Printf("[SSHD] Listen failed for %s %s", addr, err)
				req.Reply(false, []byte{})
				continue
			}
			var replyPayload = struct{ Port uint32 }{lport}
			// Tell client everything is OK
			req.Reply(true, ssh.Marshal(replyPayload))
			go handleTcpIpForwardSession(s.client, ln, laddr, lport)
			s.forwards[addr] = ln

		case "cancel-tcpip-forward":
			var payload = struct {
				Addr string
				Port uint32
			}{}
			if err := ssh.Unmarshal(req.Payload, &payload); err != nil {
				log.Printf("[SSHD] Unable to unmarshal payload")
				req.Reply(false, []byte{})
				continue
			}
			laddr := payload.Addr
			lport := payload.Port
			addr := fmt.Sprintf("[%s]:%d", laddr, lport)
			ln, ok := s.forwards[addr]
			if ok {
				ln.Close()
			}
		default:
			if strings.Contains(req.Type, "keepalive") {
				req.Reply(true, nil)
				continue
			}
			log.Printf("[SSHD] received out-of-band request: %+v", req)
		}
	}
}

func (s *SshServer) handleChannels(chans <-chan ssh.NewChannel) {
	// Service the incoming Channel channel.
	for newChannel := range chans {
		t := newChannel.ChannelType()
		switch t {
		case "session":
			go handleChannelSession(newChannel)

		case "direct-tcpip":
			go handleChannelDirect(newChannel)
		default:
			newChannel.Reject(ssh.UnknownChannelType, fmt.Sprintf("unknown channel type: %s", t))
		}
	}
}
