package sshd

import (
	"fmt"
	"io/ioutil"
	"net"
	"runtime"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/ferama/rospo/pkg/logger"
	"github.com/ferama/rospo/pkg/utils"

	"golang.org/x/crypto/ssh"
)

var log = logger.NewLogger("[SSHD] ", logger.Blue)

// sshServer instance
type sshServer struct {
	hostPrivateKey    ssh.Signer
	authorizedKeyFile *string
	password          string
	listenAddress     *string

	disableShell bool

	forwards   map[string]net.Listener
	forwardsMu sync.Mutex

	forwardsKeepAliveInterval time.Duration

	listener   net.Listener
	listenerMU sync.RWMutex
}

// NewSshServer builds an SshServer object
func NewSshServer(conf *SshDConf) *sshServer {
	keyPath, _ := utils.ExpandUserHome(conf.Key)
	hostPrivateKey, err := ioutil.ReadFile(keyPath)
	if err != nil {
		log.Println("server identity do not exists. Generating one...")
		key, err := utils.GeneratePrivateKey()
		if err != nil {
			panic(err)
		}
		encoded := utils.EncodePrivateKeyToPEM(key)
		if err := utils.WriteKeyToFile(encoded, keyPath); err != nil {
			panic(err)
		}
		hostPrivateKey = encoded

		// this is the one to use in the known_hosts file
		publicKey, err := utils.GeneratePublicKey(&key.PublicKey)
		if err != nil {
			panic(err)
		}
		utils.WriteKeyToFile(publicKey, keyPath+".pub")
	}

	hostPrivateKeySigner, err := ssh.ParsePrivateKey(hostPrivateKey)
	if err != nil {
		panic(err)
	}

	ss := &sshServer{
		authorizedKeyFile:         &conf.AuthorizedKeysFile,
		password:                  conf.AuthorizedPassword,
		hostPrivateKey:            hostPrivateKeySigner,
		disableShell:              conf.DisableShell,
		listenAddress:             &conf.ListenAddress,
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
	path, err := utils.ExpandUserHome(*s.authorizedKeyFile)
	if err != nil {
		log.Fatalln(err)
	}
	authorizedKeysBytes, err := ioutil.ReadFile(path)
	if err != nil && s.password == "" {
		log.Fatalf(`failed to load authorized_keys, err: %v

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

func (s *sshServer) passwordAuth(conn ssh.ConnMetadata, password []byte) (*ssh.Permissions, error) {
	if s.password == string(password) {
		return &ssh.Permissions{}, nil
	}
	return nil, fmt.Errorf("wrong password")
}

func (s *sshServer) keyAuth(conn ssh.ConnMetadata, pubKey ssh.PublicKey) (*ssh.Permissions, error) {
	log.Println(conn.RemoteAddr(), "authenticate with", pubKey.Type())

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
	bannerCb := func(conn ssh.ConnMetadata) string {
		return `
.-------------.
| Rospo sshd  |
.-------------.
    _    _
   (o)--(o)
  /.______.\
  \________/
 ./        \.
( .        , )
 \ \_\\//_/ /
  ~~  ~~  ~~
`
	}
	if runtime.GOOS == "windows" {
		bannerCb = nil
	}

	config := ssh.ServerConfig{
		AuthLogCallback: func(conn ssh.ConnMetadata, method string, err error) {
			if err != nil {
				log.Printf("auth error: %s", err)
			}
		},
		BannerCallback: bannerCb,
	}
	config.AddHostKey(s.hostPrivateKey)
	if *s.listenAddress == "" {
		log.Fatalf("listen port can't be empty")
	}

	// if password auth is enabled, add the required config
	if s.password != "" {
		config.PasswordCallback = s.passwordAuth
		config.MaxAuthTries = 3
	} else {
		// one try only. I'm supporting public key auth.
		// If it fails, there is nothing more to try
		config.MaxAuthTries = 1
		config.PublicKeyCallback = s.keyAuth
	}

	listener, err := net.Listen("tcp", *s.listenAddress)

	s.listenerMU.Lock()
	s.listener = listener
	s.listenerMU.Unlock()

	if err != nil {
		panic(err)
	}
	log.Printf("listening on %s\n", listener.Addr())
	for {
		conn, err := listener.Accept()
		if err != nil {
			panic(err)
		}
		log.Printf("connection from %s", conn.RemoteAddr())
		go func() {
			// From a standard TCP connection to an encrypted SSH connection
			sshConn, chans, reqs, err := ssh.NewServerConn(conn, &config)
			if err != nil {
				log.Printf("client connection error %s", err)
				return
			}
			log.Printf("logged in with key %s", sshConn.Permissions.Extensions["pubkey-fp"])

			// handle forwards and keepalive requests
			go s.handleRequests(sshConn, reqs)
			// Accept all channels
			go s.handleChannels(chans)
		}()
	}
}

// GetListenerAddr returns the server listener network address
func (s *sshServer) GetListenerAddr() net.Addr {
	s.listenerMU.RLock()
	defer s.listenerMU.RUnlock()

	if s.listener != nil {
		return s.listener.Addr()
	}
	return nil
}

func (s *sshServer) handleRequests(sshConn *ssh.ServerConn, reqs <-chan *ssh.Request) {
	for req := range reqs {
		switch req.Type {
		case "tcpip-forward":
			var payload = struct {
				Addr string
				Port uint32
			}{}
			if err := ssh.Unmarshal(req.Payload, &payload); err != nil {
				log.Printf("Unable to unmarshal payload")
				req.Reply(false, []byte{})
				continue
			}
			laddr := payload.Addr
			lport := payload.Port
			addr := fmt.Sprintf("[%s]:%d", laddr, lport)

			ln, err := net.Listen("tcp", addr)
			if err != nil {
				log.Printf("listen failed for %s %s", addr, err)
				req.Reply(false, []byte{})
				continue
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
			go handleTcpIpForwardSession(sshConn, ln, laddr, lport)

			go s.checkAlive(sshConn, ln, addr)

			s.forwardsMu.Lock()
			s.forwards[addr] = ln
			s.forwardsMu.Unlock()

		case "cancel-tcpip-forward":
			var payload = struct {
				Addr string
				Port uint32
			}{}
			if err := ssh.Unmarshal(req.Payload, &payload); err != nil {
				log.Printf("Unable to unmarshal payload")
				req.Reply(false, []byte{})
				continue
			}
			// TODO: what happens here if the original port was 0 (random port)?
			laddr := payload.Addr
			lport := payload.Port
			addr := fmt.Sprintf("[%s]:%d", laddr, lport)
			s.forwardsMu.Lock()
			ln, ok := s.forwards[addr]
			s.forwardsMu.Unlock()
			if ok {
				ln.Close()
			}
			req.Reply(true, nil)
		default:
			if strings.Contains(req.Type, "keepalive") {
				req.Reply(true, nil)
				continue
			}
			log.Printf("received out-of-band request: %+v", req)
		}
	}
}

func (s *sshServer) checkAlive(sshConn *ssh.ServerConn, ln net.Listener, addr string) {
	ticker := time.NewTicker(s.forwardsKeepAliveInterval)

	log.Println("starting check for forward availability")
	for {
		<-ticker.C
		_, _, err := sshConn.SendRequest("checkalive@rospo", true, nil)
		if err != nil {
			log.Printf("forward endpoint not available anymore. Closing socket %s", ln.Addr())
			ln.Close()
			s.forwardsMu.Lock()
			delete(s.forwards, addr)
			s.forwardsMu.Unlock()
			return
		}
	}
}

func (s *sshServer) handleChannels(chans <-chan ssh.NewChannel) {
	// Service the incoming Channel channel.
	for newChannel := range chans {
		t := newChannel.ChannelType()
		switch t {
		case "session":
			go handleChannelSession(newChannel, s.disableShell)

		case "direct-tcpip":
			go handleChannelDirect(newChannel)
		default:
			newChannel.Reject(ssh.UnknownChannelType, fmt.Sprintf("unknown channel type: %s", t))
		}
	}
}
