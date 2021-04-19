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

	"github.com/ferama/rospo/conf"
	"github.com/ferama/rospo/utils"
	"golang.org/x/crypto/ssh"
	"golang.org/x/crypto/ssh/knownhosts"
)

// SshClient implements an ssh client
type SshClient struct {
	username string
	identity string

	serverEndpoint *utils.Endpoint

	Client    *ssh.Client
	insecure  bool
	jumpHosts []conf.JumpHostConf

	reconnectionInterval time.Duration
	keepAliveInterval    time.Duration

	// used to tell the tunnels if this sshClient
	// is Connected. Tunnels will wait on this waitGroup to
	// know if the ssh client is Connected or no
	Connected sync.WaitGroup
}

// NewSshClient creates a new SshClient instance
func NewSshClient(conf *conf.SshClientConf) *SshClient {
	c := &SshClient{
		username:       conf.Username,
		identity:       conf.Identity,
		serverEndpoint: conf.GetServerEndpoint(),
		insecure:       conf.Insecure,
		jumpHosts:      conf.JumpHosts,

		keepAliveInterval:    5 * time.Second,
		reconnectionInterval: 5 * time.Second,
	}
	// client not connected on startup, so add 1 here
	c.Connected.Add(1)
	return c
}

// Close closes the ssh client instance connection
func (s *SshClient) Close() {
	s.Client.Close()
}

// Start connects the ssh client to the remote server
// and keeps it connected sending keep alive packet
// and reconnecting in the event of network failures
func (s *SshClient) Start() {
	for {
		if err := s.connect(); err != nil {
			log.Printf("[TUN] error while connecting %s", err)
			time.Sleep(s.reconnectionInterval)
			continue
		}
		// client connected. Free the wait group
		s.Connected.Done()
		s.keepAlive()
		s.Close()
		s.Connected.Add(1)
	}
}

func (s *SshClient) keepAlive() {
	log.Println("[TUN] starting client keep alive")
	for {
		// log.Println("[TUN] keep alive")
		_, _, err := s.Client.SendRequest("keepalive@rospo", true, nil)
		if err != nil {
			log.Printf("[TUN] error while sending keep alive %s", err)
			return
		}
		time.Sleep(s.keepAliveInterval)
	}
}
func (s *SshClient) connect() error {
	// refer to https://godoc.org/golang.org/x/crypto/ssh for other authentication types
	sshConfig := &ssh.ClientConfig{
		// SSH connection username
		User: s.username,
		Auth: []ssh.AuthMethod{
			utils.PublicKeyFile(s.identity),
		},
		HostKeyCallback: s.verifyHostCallback(),
	}
	log.Println("[TUN] trying to connect to remote server...")

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

func (s *SshClient) verifyHostCallback() ssh.HostKeyCallback {

	if s.insecure {
		return func(host string, remote net.Addr, key ssh.PublicKey) error {
			return nil
		}
	}
	return func(host string, remote net.Addr, key ssh.PublicKey) error {
		var err error
		usr, err := user.Current()
		if err != nil {
			log.Fatalf("[TUN] could not obtain user home directory :%v", err)
		}

		knownHostFile := filepath.Join(usr.HomeDir, ".ssh", "known_hosts")
		log.Printf("[TUN] known_hosts file used: %s", knownHostFile)

		clb, err := knownhosts.New(knownHostFile)
		if err != nil {
			log.Printf("[TUN] error while parsing 'known_hosts' file: %s: %v", knownHostFile, err)
			f, fErr := os.OpenFile(knownHostFile, os.O_CREATE, 0600)
			if fErr != nil {
				log.Fatalf("[TUN] %s", fErr)
			}
			f.Close()
			clb, err = knownhosts.New(knownHostFile)
			if err != nil {
				log.Fatalf("[TUN] %s", err)
			}
		}
		var keyErr *knownhosts.KeyError
		e := clb(host, remote, key)
		if errors.As(e, &keyErr) && len(keyErr.Want) > 0 {
			log.Printf("[TUN] ERROR: %v is not a key of %s, either a man in the middle attack or %s host pub key was changed.", key, host, host)
			return e
		} else if errors.As(e, &keyErr) && len(keyErr.Want) == 0 {
			log.Printf("[TUN] WARNING: %s is not trusted, adding this key: \n\n%s\n\nto known_hosts file.", host, utils.SerializeKey(key))
			return utils.AddHostKeyToKnownHosts(host, key)
		}
		return e
	}
}

func (s *SshClient) jumpHostConnect(
	server *utils.Endpoint,
	sshConfig *ssh.ClientConfig,
) (*ssh.Client, error) {

	jhost := s.jumpHosts[0]
	jhostParsed := utils.ParseSSHUrl(jhost.URI)
	proxyConfig := &ssh.ClientConfig{
		// SSH connection username
		User: jhostParsed.Username,
		Auth: []ssh.AuthMethod{
			utils.PublicKeyFile(jhost.Identity),
		},
		HostKeyCallback: s.verifyHostCallback(),
	}

	proxyClient, err := s.directConnect(&utils.Endpoint{
		Host: jhostParsed.Host,
		Port: jhostParsed.Port,
	}, proxyConfig)
	if err != nil {
		return nil, err
	}
	log.Println("[TUN] reached the jump host")

	log.Printf("[TUN] connecting to %s", server.String())
	conn, err := proxyClient.Dial("tcp", server.String())
	if err != nil {
		return nil, err
	}
	log.Println("[TUN] connected to remote server")

	ncc, chans, reqs, err := ssh.NewClientConn(conn, server.String(), sshConfig)
	if err != nil {
		return nil, err
	}

	client := ssh.NewClient(ncc, chans, reqs)
	return client, nil
}

func (s *SshClient) directConnect(
	server *utils.Endpoint,
	sshConfig *ssh.ClientConfig,
) (*ssh.Client, error) {

	log.Printf("[TUN] connecting to %s", server.String())
	client, err := ssh.Dial("tcp", server.String(), sshConfig)
	if err != nil {
		log.Printf("[TUN] dial INTO remote server error. %s", err)
		return nil, err
	}
	log.Println("[TUN] connected to remote server.")
	return client, nil
}
