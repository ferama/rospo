package tun

import (
	"fmt"
	"log"
	"net"
	"os/user"
	"path/filepath"
	"time"

	"github.com/ferama/rospo/utils"

	"golang.org/x/crypto/ssh"
	"golang.org/x/crypto/ssh/knownhosts"
)

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

	// the tunnell connection listener
	listener net.Listener
}

func NewTunnel(
	username string,
	identity string,
	serverEndpoint *Endpoint,
	remoteEndpoint *Endpoint,
	localEndpoint *Endpoint,
	jumpHost string,
	isForward bool,
	insecure bool,
) *Tunnel {

	tunnel := &Tunnel{
		forward:        isForward,
		insecure:       insecure,
		jumpHost:       jumpHost,
		username:       username,
		identity:       identity,
		serverEndpoint: serverEndpoint,
		remoteEndpoint: remoteEndpoint,
		localEndpoint:  localEndpoint,

		stopKeepAlive:        make(chan bool),
		keepAliveInterval:    1 * time.Second,
		reconnectionInterval: 5 * time.Second,
	}

	return tunnel
}

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
		// HostKeyCallback: ssh.InsecureIgnoreHostKey(),
		HostKeyCallback: t.verifyHostCallback(),
	}
	log.Println("[TUN] trying to connect to remote server...")

	if t.jumpHost != "" {
		jhostParsed := utils.ParseSSHUrl(t.jumpHost)
		proxyConfig := &ssh.ClientConfig{
			// SSH connection username
			User: jhostParsed.Username,
			Auth: []ssh.AuthMethod{
				utils.PublicKeyFile(t.identity),
				// ssh.Password("your_password_here"),
			},
			// HostKeyCallback: ssh.InsecureIgnoreHostKey(),
			HostKeyCallback: t.verifyHostCallback(),
		}
		jumpHostService := fmt.Sprintf("%s:%d", jhostParsed.Host, jhostParsed.Port)
		proxyClient, err := ssh.Dial("tcp", jumpHostService, proxyConfig)
		if err != nil {
			return err
		}
		log.Println("[TUN] reached the jump host")

		log.Printf("[TUN] connecting to %s", t.serverEndpoint.String())
		conn, err := proxyClient.Dial("tcp", t.serverEndpoint.String())
		if err != nil {
			return err
		}
		log.Println("[TUN] connected to remote server")

		ncc, chans, reqs, err := ssh.NewClientConn(conn, t.serverEndpoint.String(), sshConfig)
		if err != nil {
			return err
		}

		client := ssh.NewClient(ncc, chans, reqs)
		t.client = client

	} else {
		// Connect to SSH remote server using serverEndpoint
		log.Printf("[TUN] connecting to %s", t.serverEndpoint.String())
		client, err := ssh.Dial("tcp", t.serverEndpoint.String(), sshConfig)
		if err != nil {
			log.Println(fmt.Printf("[TUN] Dial INTO remote server error. %s\n", err))
			return err
		}
		log.Println("[TUN] connected to remote server.")

		t.client = client
	}

	return nil
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
		return func(hostname string, remote net.Addr, key ssh.PublicKey) error {
			return nil
		}
	}

	var err error
	usr, err := user.Current()
	if err != nil {
		log.Fatalf("could not obtain user home directory :%v", err)
	}

	knownHostFile := filepath.Join(usr.HomeDir, ".ssh", "known_hosts")
	log.Printf("[TUN] known_hosts file used: %s", knownHostFile)

	clb, err := knownhosts.New(knownHostFile)
	if err != nil {
		log.Fatalf("error while parsing 'known_hosts' file: %s: %v", knownHostFile, err)
	}
	return clb
}
