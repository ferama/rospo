package tun

import (
	"errors"
	"fmt"
	"log"
	"net"
	"os"
	"os/user"
	"path/filepath"
	"time"

	"github.com/ferama/rospo/utils"

	"golang.org/x/crypto/ssh"
	"golang.org/x/crypto/ssh/knownhosts"
)

// Tunnel object
type Tunnel struct {
	// indicates if it is a forward or reverse tunnel
	forward bool

	insecure bool

	jumpHost string

	username string
	identity string

	serverEndpoint *Endpoint
	remoteEndpoint *Endpoint
	localEndpoint  *Endpoint

	client *ssh.Client

	stopKeepAlive        chan bool
	keepAliveInterval    time.Duration
	reconnectionInterval time.Duration

	// the tunnel connection listener
	listener net.Listener
}

// NewTunnel builds a Tunnel object
func NewTunnel(conf *Config) *Tunnel {

	tunnel := &Tunnel{
		forward:        conf.Forward,
		insecure:       conf.Insecure,
		jumpHost:       conf.JumpHost,
		username:       conf.Username,
		identity:       conf.Identity,
		serverEndpoint: conf.GetServerEndpoint(),
		remoteEndpoint: conf.GetRemotEndpoint(),
		localEndpoint:  conf.GetLocalEndpoint(),

		stopKeepAlive:        make(chan bool),
		keepAliveInterval:    5 * time.Second,
		reconnectionInterval: 5 * time.Second,
	}

	return tunnel
}

// Start activates the tunnel connections
func (t *Tunnel) Start() {

	for {
		if err := t.connectToServer(); err != nil {
			log.Printf("[TUN] error while connecting %s", err)
			time.Sleep(t.reconnectionInterval)
			continue
		}

		// start the keepAlive routine
		go t.keepAlive()

		if t.forward {
			t.listenLocal()
		} else {
			t.listenRemote()
		}
		t.stopKeepAlive <- true
		t.client.Close()

		time.Sleep(t.reconnectionInterval)
	}
}

func (t *Tunnel) connectToServer() error {
	// refer to https://godoc.org/golang.org/x/crypto/ssh for other authentication types
	sshConfig := &ssh.ClientConfig{
		// SSH connection username
		User: t.username,
		Auth: []ssh.AuthMethod{
			utils.PublicKeyFile(t.identity),
			// ssh.Password("your_password_here"),
		},
		HostKeyCallback: t.verifyHostCallback(),
	}
	log.Println("[TUN] trying to connect to remote server...")

	if t.jumpHost != "" {
		client, err := t.jumpHostConnect(sshConfig)
		if err != nil {
			return err
		}
		t.client = client

	} else {
		client, err := t.directConnect(sshConfig)
		if err != nil {
			return err
		}
		t.client = client
	}

	return nil
}
func (t *Tunnel) jumpHostConnect(sshConfig *ssh.ClientConfig) (*ssh.Client, error) {
	jhostParsed := utils.ParseSSHUrl(t.jumpHost)
	proxyConfig := &ssh.ClientConfig{
		// SSH connection username
		User: jhostParsed.Username,
		Auth: []ssh.AuthMethod{
			utils.PublicKeyFile(t.identity),
			// ssh.Password("your_password_here"),
		},
		HostKeyCallback: t.verifyHostCallback(),
	}

	jumpHostService := fmt.Sprintf("%s:%d", jhostParsed.Host, jhostParsed.Port)
	proxyClient, err := ssh.Dial("tcp", jumpHostService, proxyConfig)
	if err != nil {
		return nil, err
	}
	log.Println("[TUN] reached the jump host")

	log.Printf("[TUN] connecting to %s", t.serverEndpoint.String())
	conn, err := proxyClient.Dial("tcp", t.serverEndpoint.String())
	if err != nil {
		return nil, err
	}
	log.Println("[TUN] connected to remote server")

	ncc, chans, reqs, err := ssh.NewClientConn(conn, t.serverEndpoint.String(), sshConfig)
	if err != nil {
		return nil, err
	}

	client := ssh.NewClient(ncc, chans, reqs)
	return client, nil
}

func (t *Tunnel) directConnect(sshConfig *ssh.ClientConfig) (*ssh.Client, error) {
	log.Printf("[TUN] connecting to %s", t.serverEndpoint.String())
	client, err := ssh.Dial("tcp", t.serverEndpoint.String(), sshConfig)
	if err != nil {
		log.Printf("[TUN] dial INTO remote server error. %s", err)
		return nil, err
	}
	log.Println("[TUN] connected to remote server.")
	return client, nil
}

func (t *Tunnel) listenLocal() {
	// Listen on remote server port
	listener, err := net.Listen("tcp", t.localEndpoint.String())
	if err != nil {
		log.Println(fmt.Printf("[TUN] dial INTO remote service error. %s\n", err))
		return
	}
	t.listener = listener

	log.Printf("[TUN] forward connected. Local: %s <- Remote: %s\n", t.localEndpoint.String(), t.remoteEndpoint.String())
	if t.client != nil && listener != nil {
		for {
			remote, err := t.client.Dial("tcp", t.remoteEndpoint.String())
			// Open a (local) connection to localEndpoint whose content will be forwarded so serverEndpoint
			if err != nil {
				log.Println(fmt.Printf("[TUN] listen open port ON local server error. %s\n", err))
				break
			}
			client, err := listener.Accept()
			if err != nil {
				log.Println("[TUN] disconnected")
				break
			}
			serveClient(client, remote)
		}
		listener.Close()
	}
}

func (t *Tunnel) listenRemote() {
	// Listen on remote server port
	listener, err := t.client.Listen("tcp", t.remoteEndpoint.String())
	if err != nil {
		log.Println(fmt.Printf("[TUN] listen open port ON remote server error. %s\n", err))
		return
	}
	t.listener = listener

	log.Printf("[TUN] reverse connected. Local: %s -> Remote: %s\n", t.localEndpoint.String(), t.remoteEndpoint.String())
	if t.client != nil && listener != nil {
		for {
			// Open a (local) connection to localEndpoint whose content will be forwarded so serverEndpoint
			local, err := net.Dial("tcp", t.localEndpoint.String())
			if err != nil {
				log.Println(fmt.Printf("[TUN] dial INTO local service error. %s\n", err))
				break
			}

			client, err := listener.Accept()
			if err != nil {
				log.Println("[TUN] disconnected")
				break
			}
			serveClient(client, local)
		}
		listener.Close()
	}
}

func (t *Tunnel) keepAlive() {
	ticker := time.NewTicker(t.keepAliveInterval)

	log.Println("[TUN] starting keep alive")
	for {
		select {
		case <-ticker.C:
			// log.Println("[TUN] keep alive")
			_, _, err := t.client.SendRequest("keepalive@rospo", true, nil)
			if err != nil {
				log.Printf("[TUN] error while sending keep alive %s", err)
				t.listener.Close()
			}
		case <-t.stopKeepAlive:
			log.Println("[TUN] keep alive stopped")
			return
		}
	}
}

func (t *Tunnel) verifyHostCallback() ssh.HostKeyCallback {

	if t.insecure {
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
