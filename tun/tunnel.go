package tun

import (
	"fmt"
	"gotun/utils"
	"log"
	"net"
	"time"

	"golang.org/x/crypto/ssh"
)

type Tunnel struct {
	// indicates if it is a forward or reverse tunnel
	forward bool

	username string
	identity string

	serverEndpoint *Endpoint
	remoteEndpoint *Endpoint
	localEndpoint  *Endpoint

	serverConn *ssh.Client

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
	isForward bool,
) *Tunnel {

	tunnel := &Tunnel{
		forward:        isForward,
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
		// t.stopKeepAlive = make(chan bool)
		// refer to https://godoc.org/golang.org/x/crypto/ssh for other authentication types
		sshConfig := &ssh.ClientConfig{
			// SSH connection username
			User: t.username,
			Auth: []ssh.AuthMethod{
				utils.PublicKeyFile(t.identity),
				// ssh.Password("your_password_here"),
			},
			HostKeyCallback: ssh.InsecureIgnoreHostKey(),
		}
		log.Println("[TUN] Trying to connect to remote server...")
		// Connect to SSH remote server using serverEndpoint
		serverConn, err := ssh.Dial("tcp", t.serverEndpoint.String(), sshConfig)
		if err != nil {
			log.Println(fmt.Printf("[TUN] Dial INTO remote server error. %s\n", err))
			time.Sleep(t.reconnectionInterval)
			continue
		}
		log.Println("[TUN] connected to remote server.")

		t.serverConn = serverConn

		go t.keepAlive()

		if t.forward {
			t.listenLocal()
		} else {
			t.listenRemote()
		}
		t.stopKeepAlive <- true
		t.serverConn.Close()

		time.Sleep(t.reconnectionInterval)
	}
}

func (t *Tunnel) listenLocal() {
	// Listen on remote server port
	listener, err := net.Listen("tcp", t.localEndpoint.String())
	if err != nil {
		log.Println(fmt.Printf("[TUN] Dial INTO remote service error. %s\n", err))
		return
	}
	t.listener = listener

	log.Printf("[TUN] Forward connected. Local: %s <- Remote: %s\n", t.localEndpoint.String(), t.remoteEndpoint.String())
	if t.serverConn != nil && listener != nil {
		for {
			remote, err := t.serverConn.Dial("tcp", t.remoteEndpoint.String())
			// Open a (local) connection to localEndpoint whose content will be forwarded so serverEndpoint
			if err != nil {
				log.Println(fmt.Printf("[TUN] Listen open port ON local server error. %s\n", err))
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
	listener, err := t.serverConn.Listen("tcp", t.remoteEndpoint.String())
	if err != nil {
		log.Println(fmt.Printf("[TUN] Listen open port ON remote server error. %s\n", err))
		return
	}
	t.listener = listener

	log.Printf("[TUN] Reverse connected. Local: %s -> Remote: %s\n", t.localEndpoint.String(), t.remoteEndpoint.String())
	if t.serverConn != nil && listener != nil {
		for {
			// Open a (local) connection to localEndpoint whose content will be forwarded so serverEndpoint
			local, err := net.Dial("tcp", t.localEndpoint.String())
			if err != nil {
				log.Println(fmt.Printf("[TUN] Dial INTO local service error. %s\n", err))
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
			_, _, err := t.serverConn.SendRequest("keepalive@gotun", true, nil)
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
