package sshc

import (
	"errors"
	"net"
	"os"
	"os/user"
	"path/filepath"
	"sync"
	"time"

	"github.com/ferama/rospo/pkg/logger"
	"github.com/ferama/rospo/pkg/utils"
	"golang.org/x/crypto/ssh"
	"golang.org/x/crypto/ssh/knownhosts"
)

var log = logger.NewLogger("[SSHC] ", logger.Green)

// The ssh connection available statuses
const (
	STATUS_CONNECTING = "Connecting..."
	STATUS_CONNECTED  = "Connected"
	STATUS_CLOSED     = "Closed"
)

// SshConnection implements an ssh client
type SshConnection struct {
	username   string
	identity   string
	password   string
	knownHosts string

	serverEndpoint *utils.Endpoint

	insecure  bool
	jumpHosts []*JumpHostConf

	reconnectionInterval time.Duration
	keepAliveInterval    time.Duration

	Client *ssh.Client
	// used to inform the tunnels if this sshClient
	// is Connected. Tunnels will wait on this waitGroup to
	// know if the ssh client is Connected or not
	Connected sync.WaitGroup

	connectionStatus   string
	connectionStatusMU sync.Mutex
}

// NewSshConnection creates a new SshConnection instance
func NewSshConnection(conf *SshClientConf) *SshConnection {
	parsed := utils.ParseSSHUrl(conf.ServerURI)
	var knownHostsPath string
	if conf.KnownHosts == "" {
		usr, _ := user.Current()
		knownHostsPath = filepath.Join(usr.HomeDir, ".ssh", "known_hosts")
	} else {
		knownHostsPath, _ = utils.ExpandUserHome(conf.KnownHosts)
	}

	c := &SshConnection{
		username:       parsed.Username,
		identity:       conf.Identity,
		password:       conf.Password,
		knownHosts:     knownHostsPath,
		serverEndpoint: conf.GetServerEndpoint(),
		insecure:       conf.Insecure,
		jumpHosts:      conf.JumpHosts,

		keepAliveInterval:    5 * time.Second,
		reconnectionInterval: 5 * time.Second,
		connectionStatus:     STATUS_CONNECTING,
	}
	// client is not connected on startup, so add 1 here
	c.Connected.Add(1)
	return c
}

// Close closes the ssh conn instance client connection
func (s *SshConnection) Close() {
	if s.Client != nil {
		s.Client.Close()
	}
	s.connectionStatusMU.Lock()
	s.connectionStatus = STATUS_CLOSED
	s.connectionStatusMU.Unlock()
}

// Start connects the ssh client to the remote server
// and keeps it connected sending keep alive packet
// and reconnecting in the event of network failures
func (s *SshConnection) Start() {
	for {
		s.connectionStatusMU.Lock()
		s.connectionStatus = STATUS_CONNECTING
		s.connectionStatusMU.Unlock()
		if err := s.connect(); err != nil {
			log.Printf("error while connecting %s", err)
			time.Sleep(s.reconnectionInterval)
			continue
		}
		// client connected. Free the wait group
		s.Connected.Done()
		s.connectionStatusMU.Lock()
		s.connectionStatus = STATUS_CONNECTED
		s.connectionStatusMU.Unlock()
		s.keepAlive()
		s.Close()
		s.Connected.Add(1)
	}
}

// GetConnectionStatus returns the current connection status as a string
func (s *SshConnection) GetConnectionStatus() string {
	s.connectionStatusMU.Lock()
	defer s.connectionStatusMU.Unlock()
	return s.connectionStatus
}

// GrabPubKey is an helper function that gets server pubkey
func (s *SshConnection) GrabPubKey() {
	sshConfig := &ssh.ClientConfig{
		HostKeyCallback: s.verifyHostCallback(false),
	}
	// ignore return values here. I'm using it just to trigger the
	// verifyHostCallback
	ssh.Dial("tcp", s.serverEndpoint.String(), sshConfig)
}

func (s *SshConnection) keepAlive() {
	log.Println("starting client keep alive")
	for {
		// log.Println("keep alive")
		_, _, err := s.Client.SendRequest("keepalive@rospo", true, nil)
		if err != nil {
			log.Printf("error while sending keep alive %s", err)
			return
		}
		time.Sleep(s.keepAliveInterval)
	}
}
func (s *SshConnection) connect() error {
	authMethods := []ssh.AuthMethod{}

	// log.Printf("using identity at %s", s.identity)
	// refer to https://godoc.org/golang.org/x/crypto/ssh for other authentication types
	keysAuth, err := utils.LoadIdentityFile(s.identity)
	if err == nil {
		authMethods = append(authMethods, keysAuth)
	} else {
		if s.password == "" {
			log.Fatal("No usable auth method defined")
		}
		authMethods = append(authMethods, ssh.Password(s.password))
	}
	sshConfig := &ssh.ClientConfig{
		// SSH connection username
		User:            s.username,
		Auth:            authMethods,
		HostKeyCallback: s.verifyHostCallback(true),
	}
	log.Println("trying to connect to remote server...")

	if len(s.jumpHosts) != 0 {
		client, err := s.jumpHostConnect(s.serverEndpoint, sshConfig)
		if err != nil {
			return err
		}
		s.Client = client

	} else {
		client, err := s.directConnect(s.serverEndpoint, sshConfig)
		if err != nil {
			return err
		}
		s.Client = client
	}

	return nil
}

func (s *SshConnection) verifyHostCallback(fail bool) ssh.HostKeyCallback {

	if s.insecure {
		return func(host string, remote net.Addr, key ssh.PublicKey) error {
			return nil
		}
	}
	return func(host string, remote net.Addr, key ssh.PublicKey) error {
		var err error

		log.Printf("known_hosts file used: %s", s.knownHosts)

		clb, err := knownhosts.New(s.knownHosts)
		if err != nil {
			log.Printf("error while parsing 'known_hosts' file: %s: %v", s.knownHosts, err)
			f, fErr := os.OpenFile(s.knownHosts, os.O_CREATE, 0600)
			if fErr != nil {
				log.Fatalf("%s", fErr)
			}
			f.Close()
			clb, err = knownhosts.New(s.knownHosts)
			if err != nil {
				log.Fatalf("%s", err)
			}
		}
		var keyErr *knownhosts.KeyError
		e := clb(host, remote, key)
		if errors.As(e, &keyErr) && len(keyErr.Want) > 0 {
			log.Printf("ERROR: %v is not a key of %s, either a man in the middle attack or %s host pub key was changed.", key, host, host)
			return e
		} else if errors.As(e, &keyErr) && len(keyErr.Want) == 0 {
			if fail {
				log.Fatalf(`ERROR: the host '%s' is not trusted. If it is trusted instead, 
				  please grab its pub key using the 'rospo grabpubkey' command`, host)
				return errors.New("")
			}
			log.Printf("WARNING: %s is not trusted, adding this key: \n\n%s\n\nto known_hosts file.", host, utils.SerializePublicKey(key))
			return utils.AddHostKeyToKnownHosts(host, key, s.knownHosts)
		}
		return e
	}
}

func (s *SshConnection) jumpHostConnect(
	server *utils.Endpoint,
	sshConfig *ssh.ClientConfig,
) (*ssh.Client, error) {

	var (
		jhClient *ssh.Client
		jhConn   net.Conn
		err      error
	)

	// traverse all the hops
	for idx, jh := range s.jumpHosts {
		parsed := utils.ParseSSHUrl(jh.URI)
		hop := &utils.Endpoint{
			Host: parsed.Host,
			Port: parsed.Port,
		}

		authMethods := []ssh.AuthMethod{}

		keysAuth, err := utils.LoadIdentityFile(s.identity)
		if err == nil {
			authMethods = append(authMethods, keysAuth)
		} else {
			if s.password == "" {
				log.Fatal("No usable auth method defined")
			}
			authMethods = append(authMethods, ssh.Password(s.password))
		}
		config := &ssh.ClientConfig{
			User:            parsed.Username,
			Auth:            authMethods,
			HostKeyCallback: s.verifyHostCallback(true),
		}
		log.Printf("connecting to hop %s@%s", parsed.Username, hop.String())

		// if it is the first hop, use ssh Dial to create the first client
		if idx == 0 {
			jhClient, err = ssh.Dial("tcp", hop.String(), config)
			if err != nil {
				log.Printf("dial INTO remote server error. %s", err)
				return nil, err
			}
		} else {
			jhConn, err = jhClient.Dial("tcp", hop.String())
			if err != nil {
				return nil, err
			}
			ncc, chans, reqs, err := ssh.NewClientConn(jhConn, hop.String(), config)
			if err != nil {
				return nil, err
			}
			jhClient = ssh.NewClient(ncc, chans, reqs)
		}
		log.Printf("reached the jump host %s@%s", parsed.Username, hop.String())
	}

	// now I'm ready to reach the final hop, the server
	log.Printf("connecting to %s@%s", sshConfig.User, server.String())
	jhConn, err = jhClient.Dial("tcp", server.String())
	if err != nil {
		return nil, err
	}
	ncc, chans, reqs, err := ssh.NewClientConn(jhConn, server.String(), sshConfig)
	if err != nil {
		return nil, err
	}
	client := ssh.NewClient(ncc, chans, reqs)

	return client, nil
}

func (s *SshConnection) directConnect(
	server *utils.Endpoint,
	sshConfig *ssh.ClientConfig,
) (*ssh.Client, error) {

	log.Printf("connecting to %s", server.String())
	client, err := ssh.Dial("tcp", server.String(), sshConfig)
	if err != nil {
		log.Printf("dial INTO remote server error. %s", err)
		return nil, err
	}
	log.Println("connected to remote server.")
	return client, nil
}
