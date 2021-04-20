package sshd

import (
	"fmt"
	"io/ioutil"
	"log"
	"net"
	"strings"
	"time"

	"github.com/ferama/rospo/conf"
	"github.com/ferama/rospo/utils"

	"golang.org/x/crypto/ssh"
)

// sshServer instance
type sshServer struct {
	client            *ssh.ServerConn
	hostPrivateKey    ssh.Signer
	authorizedKeyFile *string
	tcpPort           *string

	forwards map[string]net.Listener

	forwardsKeepAliveInterval time.Duration
}

// NewSshServer builds an SshServer object
// func NewSshServer(identity *string, authorizedKeys *string, tcpPort *string) *sshServer {
func NewSshServer(conf *conf.SshDConf) *sshServer {
	hostPrivateKey, err := ioutil.ReadFile(conf.Key)
	if err != nil {
		log.Println("[SSHD] server identity do not exists. Generating one...")
		utils.GeneratePrivateKey(&conf.Key)
		hostPrivateKey, err = ioutil.ReadFile(conf.Key)
		if err != nil {
			panic(err)
		}
	}

	hostPrivateKeySigner, err := ssh.ParsePrivateKey(hostPrivateKey)
	if err != nil {
		panic(err)
	}

	ss := &sshServer{
		authorizedKeyFile:         &conf.AuthorizedKeysFile,
		hostPrivateKey:            hostPrivateKeySigner,
		tcpPort:                   &conf.Port,
		forwards:                  make(map[string]net.Listener),
		forwardsKeepAliveInterval: 5 * time.Second,
	}

	// run here, to make sure I have a valid authorized keys
	// file on start
	ss.loadAuthorizedKeys()

	return ss
}

func (s *sshServer) loadAuthorizedKeys() map[string]bool {
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

func (s *sshServer) keyAuth(conn ssh.ConnMetadata, pubKey ssh.PublicKey) (*ssh.Permissions, error) {
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

// Start the sshServer actually listening for incoming connections
// and handling requests and ssh channels
func (s *sshServer) Start() {
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
	if *s.tcpPort == "" {
		log.Fatalf("[SSHD] listen port can't be empty")
	}

	socket, err := net.Listen("tcp", ":"+*s.tcpPort)
	if err != nil {
		panic(err)
	}
	log.Printf("[SSHD] listening on port %s\n", *s.tcpPort)
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

			log.Println("[SSHD] connection from", sshConn.RemoteAddr())
			// handle forwards and keepalive requests
			go s.handleRequests(reqs)
			// Accept all channels
			go s.handleChannels(chans)
		}()
	}
}

func (s *sshServer) handleRequests(reqs <-chan *ssh.Request) {
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
				log.Printf("[SSHD] listen failed for %s %s", addr, err)
				req.Reply(false, []byte{})
				continue
			}
			log.Printf("[SSHD] tcpip-forward listening  for %s", addr)
			var replyPayload = struct{ Port uint32 }{lport}
			// Tell client everything is OK
			req.Reply(true, ssh.Marshal(replyPayload))
			go handleTcpIpForwardSession(s.client, ln, laddr, lport)

			go func(s *sshServer, addr string) {
				ticker := time.NewTicker(s.forwardsKeepAliveInterval)

				log.Println("[SSHD] starting check for forward availability")
				stopKeepAlive := make(chan bool)
				for {
					select {
					case <-ticker.C:
						_, _, err := s.client.SendRequest("checkalive@rospo", true, nil)
						if err != nil {
							log.Println("[SSHD] forward endpoint not available anymore. Closing socket")
							ln.Close()
							stopKeepAlive <- true
							delete(s.forwards, addr)
						}
					case <-stopKeepAlive:
						return
					}
				}
			}(s, addr)

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

func (s *sshServer) handleChannels(chans <-chan ssh.NewChannel) {
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
