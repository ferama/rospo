package sshd

import (
	"fmt"
	"gotun/utils"
	"io/ioutil"
	"log"
	"net"

	"golang.org/x/crypto/ssh"
)

var (
	DEFAULT_SHELL string = "sh"
)

var (
	hostPrivateKeySigner ssh.Signer
)

type SshServer struct {
	authorizedKeyFile *string
	client            *ssh.ServerConn
	tcpPort           *string
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

	hostPrivateKeySigner, err = ssh.ParsePrivateKey(hostPrivateKey)
	if err != nil {
		panic(err)
	}

	ss := &SshServer{
		authorizedKeyFile: authorizedKeys,
		tcpPort:           tcpPort,
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
				log.Printf("[SSHD] Auth error: %s", err)
			}
		},
	}
	config.AddHostKey(hostPrivateKeySigner)

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
		if req.Type == "tcpip-forward" {
			handleTcpIpForward(req, s.client)
			continue
		}
		log.Printf("[SSHD] recieved out-of-band request: %+v", req)
	}
}

func (s *SshServer) handleChannels(chans <-chan ssh.NewChannel) {
	// Service the incoming Channel channel.
	for newChannel := range chans {
		t := newChannel.ChannelType()
		if t == "session" {
			go handleChannelSession(newChannel)
			continue
		}
		if t == "direct-tcpip" {
			go handleChannelDirect(newChannel)
			continue
		}
		newChannel.Reject(ssh.UnknownChannelType, fmt.Sprintf("unknown channel type: %s", t))
	}
}
