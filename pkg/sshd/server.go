package sshd

import (
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"os"
	"runtime"
	"sync"

	"github.com/ferama/rospo/pkg/logger"
	"github.com/ferama/rospo/pkg/utils"

	"golang.org/x/crypto/ssh"
)

var log = logger.NewLogger("[SSHD] ", logger.Blue)

// sshServer instance
type sshServer struct {
	hostPrivateKey    ssh.Signer
	authorizedKeysURI []string
	password          string
	listenAddress     *string

	disableShell         bool
	disableAuth          bool
	disableBanner        bool
	disableSftpSubsystem bool

	shellExecutable string

	listener   net.Listener
	listenerMU sync.RWMutex

	activeSessions  int
	activeSessionMu sync.Mutex
}

// NewSshServer builds an SshServer object
func NewSshServer(conf *SshDConf) *sshServer {
	keyPath, _ := utils.ExpandUserHome(conf.Key)
	if keyPath == "" {
		log.Fatalln("server_key is not set")
	}
	log.Printf("loading server key at: '%s'", keyPath)
	hostPrivateKey, err := os.ReadFile(keyPath)
	log.Printf("authorized_keys: %s", conf.AuthorizedKeysURI)
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
		log.Fatalln(err)
	}

	ss := &sshServer{
		authorizedKeysURI:    conf.AuthorizedKeysURI,
		password:             conf.AuthorizedPassword,
		hostPrivateKey:       hostPrivateKeySigner,
		shellExecutable:      conf.ShellExecutable,
		disableShell:         conf.DisableShell,
		disableBanner:        conf.DisableBanner,
		disableSftpSubsystem: conf.DisableSftpSubsystem,
		disableAuth:          conf.DisableAuth,
		listenAddress:        &conf.ListenAddress,
		activeSessions:       0,
	}
	// run here, to make sure I have a valid authorized keys
	// file on start
	if !conf.DisableAuth {
		res := ss.loadAuthorizedKeys()
		if len(res) == 0 && conf.AuthorizedPassword == "" {
			log.Fatalf(`failed to load authorized_keys, err: %v
	
	You need an authorized_keys source. You can create and 
	use an ./authorized_keys file and fill in with 
	your authorized users public keys. You can optionally use
	an http endpoint that serves your authorized_keys.
	Run "rospo sshd --help" for more info

`, err)
		}
	}

	return ss
}

func (s *sshServer) parseAuthorizedKeysBytes(bytes []byte) (map[string]bool, error) {
	authorizedKeysMap := map[string]bool{}
	authorizedKeysBytes := bytes
	for len(authorizedKeysBytes) > 0 {
		pubKey, _, _, rest, err := ssh.ParseAuthorizedKey(authorizedKeysBytes)
		if err != nil {
			return authorizedKeysMap, err
		}

		authorizedKeysMap[string(pubKey.Marshal())] = true
		authorizedKeysBytes = rest
	}
	return authorizedKeysMap, nil
}

func (s *sshServer) loadAuthorizedKeys() map[string]bool {
	res := map[string]bool{}
	mergeMap := func(m map[string]bool) {
		for k, v := range m {
			res[k] = v
		}
	}

	for _, keyURI := range s.authorizedKeysURI {
		u, err := url.ParseRequestURI(keyURI)
		if err != nil || u.Scheme == "" {
			log.Println("loading keys from file", keyURI)
			path, err := utils.ExpandUserHome(keyURI)
			if err != nil {
				continue
			}
			authorizedKeysBytes, err := os.ReadFile(path)
			if err != nil {
				continue
			}
			result, err := s.parseAuthorizedKeysBytes(authorizedKeysBytes)
			if err == nil {
				mergeMap(result)
			}
		} else {
			if u.Scheme == "http" || u.Scheme == "https" {
				log.Println("loading keys from http", keyURI)
				res, err := http.Get(u.String())
				if err != nil {
					log.Println("failed to load keys from http", err)
					continue
				}

				bytes, err := io.ReadAll(res.Body)
				if err != nil {
					log.Println("failed to read http body", err)
					continue
				}
				result, err := s.parseAuthorizedKeysBytes(bytes)
				if err == nil {
					mergeMap(result)
				}
			}
		}
	}
	return res
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

// serve sshd client connection
func (s *sshServer) serveConnection(conn net.Conn, config ssh.ServerConfig) {
	log.Printf("connection from %s", conn.RemoteAddr())
	s.activeSessionMu.Lock()
	s.activeSessions++
	s.activeSessionMu.Unlock()
	log.Printf("active sessions: %d", s.activeSessions)

	// From a standard TCP connection to an encrypted SSH connection
	sshConn, chans, reqs, err := ssh.NewServerConn(conn, &config)
	if err != nil {
		log.Printf("client connection error %s", err)
		return
	}
	if !s.disableAuth {
		log.Printf("logged in %s", sshConn.Permissions.Extensions["pubkey-fp"])
	} else {
		log.Println("logged in WITHOUT authentication")
	}

	requestHandler := newRequestHandler(sshConn, reqs)
	go requestHandler.handleRequests()

	channelHandler := newChannelHandler(
		s,
		sshConn,
		chans,
	)

	// blocks until chans is closed (session terminates)
	channelHandler.handleChannels()
	// Accept all channels
	log.Println("client session terminated")
	s.activeSessionMu.Lock()
	s.activeSessions--
	s.activeSessionMu.Unlock()
	log.Printf("active sessions: %d", s.activeSessions)
}

// Start the sshServer actually listening for incoming connections
// and handling requests and ssh channels
func (s *sshServer) Start() {
	bannerCb := func(conn ssh.ConnMetadata) string {
		return `
 .---------------.
 | 🐸 rospo sshd |
 .---------------.

`
	}
	if runtime.GOOS == "windows" || s.disableBanner {
		bannerCb = nil
	}

	config := ssh.ServerConfig{
		BannerCallback: bannerCb,
	}
	config.AddHostKey(s.hostPrivateKey)
	if *s.listenAddress == "" {
		log.Fatalf("listen port can't be empty")
	}

	if !s.disableAuth {
		// if password auth is enabled, add the required config
		if s.password != "" {
			config.PasswordCallback = s.passwordAuth
			config.MaxAuthTries = 3
		} else {
			// one try only. I'm supporting public key auth.
			// If it fails, there is nothing more to try
			config.MaxAuthTries = 1
		}
		config.PublicKeyCallback = s.keyAuth
	} else {
		config.NoClientAuth = true
	}

	listener, err := net.Listen("tcp", *s.listenAddress)

	s.listenerMU.Lock()
	s.listener = listener
	s.listenerMU.Unlock()

	if err != nil {
		log.Fatal(err)
	}
	log.Printf("listening on %s\n", listener.Addr())
	for {
		conn, err := listener.Accept()
		if err != nil {
			panic(err)
		}
		go s.serveConnection(conn, config)
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
