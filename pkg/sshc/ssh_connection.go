package sshc

import (
	"errors"
	"log"
	"net"
	"os"
	"os/user"
	"path/filepath"
	"sync"
	"time"

	"github.com/ferama/rospo/pkg/utils"
	"golang.org/x/crypto/ssh"
	"golang.org/x/crypto/ssh/knownhosts"
)

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

	ConnectionStatus string
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
		knownHosts:     knownHostsPath,
		serverEndpoint: conf.GetServerEndpoint(),
		insecure:       conf.Insecure,
		jumpHosts:      conf.JumpHosts,

		keepAliveInterval:    5 * time.Second,
		reconnectionInterval: 5 * time.Second,
		ConnectionStatus:     STATUS_CONNECTING,
	}
	// client is not connected on startup, so add 1 here
	c.Connected.Add(1)
	return c
}

// Close closes the ssh conn instance client connection
func (s *SshConnection) Close() {
	s.Client.Close()
	s.ConnectionStatus = STATUS_CLOSED
}

// Start connects the ssh client to the remote server
// and keeps it connected sending keep alive packet
// and reconnecting in the event of network failures
func (s *SshConnection) Start() {
	for {
		s.ConnectionStatus = STATUS_CONNECTING
		if err := s.connect(); err != nil {
			log.Printf("[SSHC] error while connecting %s", err)
			time.Sleep(s.reconnectionInterval)
			continue
		}
		// client connected. Free the wait group
		s.Connected.Done()
		s.ConnectionStatus = STATUS_CONNECTED
		s.keepAlive()
		s.Close()
		s.Connected.Add(1)
	}
}

// GrapPubKey is an helper function that gets server pubkey
func (s *SshConnection) GrabPubKey() {
	sshConfig := &ssh.ClientConfig{
		HostKeyCallback: s.verifyHostCallback(false),
	}
	// ignore return values here. I'm using it just to trigger the
	// verifyHostCallback
	ssh.Dial("tcp", s.serverEndpoint.String(), sshConfig)
}

func (s *SshConnection) keepAlive() {
	log.Println("[SSHC] starting client keep alive")
	for {
		// log.Println("[SSHC] keep alive")
		_, _, err := s.Client.SendRequest("keepalive@rospo", true, nil)
		if err != nil {
			log.Printf("[SSHC] error while sending keep alive %s", err)
			return
		}
		time.Sleep(s.keepAliveInterval)
	}
}
func (s *SshConnection) connect() error {
	// refer to https://godoc.org/golang.org/x/crypto/ssh for other authentication types
	sshConfig := &ssh.ClientConfig{
		// SSH connection username
		User: s.username,
		Auth: []ssh.AuthMethod{
			utils.LoadIdentityFile(s.identity),
		},
		HostKeyCallback: s.verifyHostCallback(true),
	}
	log.Println("[SSHC] trying to connect to remote server...")

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

		log.Printf("[SSHC] known_hosts file used: %s", s.knownHosts)

		clb, err := knownhosts.New(s.knownHosts)
		if err != nil {
			log.Printf("[SSHC] error while parsing 'known_hosts' file: %s: %v", s.knownHosts, err)
			f, fErr := os.OpenFile(s.knownHosts, os.O_CREATE, 0600)
			if fErr != nil {
				log.Fatalf("[SSHC] %s", fErr)
			}
			f.Close()
			clb, err = knownhosts.New(s.knownHosts)
			if err != nil {
				log.Fatalf("[SSHC] %s", err)
			}
		}
		var keyErr *knownhosts.KeyError
		e := clb(host, remote, key)
		if errors.As(e, &keyErr) && len(keyErr.Want) > 0 {
			log.Printf("[SSHC] ERROR: %v is not a key of %s, either a man in the middle attack or %s host pub key was changed.", key, host, host)
			return e
		} else if errors.As(e, &keyErr) && len(keyErr.Want) == 0 {
			if fail {
				log.Fatalf(`[SSHC] ERROR: the host '%s' is not trusted. If it is trusted instead, 
				  please grab its pub key using the 'rospo grabpubkey' command`, host)
				return errors.New("")
			}
			log.Printf("[SSHC] WARNING: %s is not trusted, adding this key: \n\n%s\n\nto known_hosts file.", host, utils.SerializePublicKey(key))
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
		config := &ssh.ClientConfig{
			User: parsed.Username,
			Auth: []ssh.AuthMethod{
				utils.LoadIdentityFile(jh.Identity),
			},
			HostKeyCallback: s.verifyHostCallback(true),
		}
		log.Printf("[SSHC] connecting to hop %s@%s", parsed.Username, hop.String())

		// if it is the first hop, use ssh Dial to create the first client
		if idx == 0 {
			jhClient, err = ssh.Dial("tcp", hop.String(), config)
			if err != nil {
				log.Printf("[SSHC] dial INTO remote server error. %s", err)
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
		log.Printf("[SSHC] reached the jump host %s@%s", parsed.Username, hop.String())
	}

	// now I'm ready to reach the final hop, the server
	log.Printf("[SSHC] connecting to %s@%s", sshConfig.User, server.String())
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

	log.Printf("[SSHC] connecting to %s", server.String())
	client, err := ssh.Dial("tcp", server.String(), sshConfig)
	if err != nil {
		log.Printf("[SSHC] dial INTO remote server error. %s", err)
		return nil, err
	}
	log.Println("[SSHC] connected to remote server.")
	return client, nil
}
