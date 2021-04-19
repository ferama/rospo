package tun

import (
	"fmt"
	"io"
	"log"
	"net"
	"sync"
	"time"

	"github.com/ferama/rospo/conf"
	"github.com/ferama/rospo/sshc"
	"github.com/ferama/rospo/utils"
)

// Tunnel object
type Tunnel struct {
	// indicates if it is a forward or reverse tunnel
	forward bool

	remoteEndpoint *utils.Endpoint
	localEndpoint  *utils.Endpoint

	sshConn              *sshc.SshConnection
	reconnectionInterval time.Duration

	// the tunnel connection listener
	listener net.Listener
}

// NewTunnel builds a Tunnel object
func NewTunnel(sshConn *sshc.SshConnection, conf *conf.TunnnelConf) *Tunnel {

	tunnel := &Tunnel{
		forward:        conf.Forward,
		remoteEndpoint: conf.GetRemotEndpoint(),
		localEndpoint:  conf.GetLocalEndpoint(),

		sshConn:              sshConn,
		reconnectionInterval: 5 * time.Second,
	}

	return tunnel
}

// Start activates the tunnel connections
func (t *Tunnel) Start() {
	for {
		// waits for the ssh client to be connected to the server
		t.sshConn.Connected.Wait()

		if t.forward {
			t.listenLocal()
		} else {
			t.listenRemote()
		}
		time.Sleep(t.reconnectionInterval)
	}
}

func (t *Tunnel) listenLocal() {
	// Listen on remote server port
	listener, err := net.Listen("tcp", t.localEndpoint.String())
	if err != nil {
		log.Printf("[TUN] dial INTO remote service error. %s\n", err)
		return
	}
	t.listener = listener

	log.Printf("[TUN] forward connected. Local: %s <- Remote: %s\n", t.localEndpoint.String(), t.remoteEndpoint.String())
	if t.sshConn != nil && listener != nil {
		for {
			remote, err := t.sshConn.Client.Dial("tcp", t.remoteEndpoint.String())
			// Open a (local) connection to localEndpoint whose content will be forwarded so serverEndpoint
			if err != nil {
				log.Printf("[TUN] listen open port ON local server error. %s\n", err)
				break
			}
			client, err := listener.Accept()
			if err != nil {
				log.Println("[TUN] disconnected")
				break
			}
			t.serveClient(client, remote)
		}
		listener.Close()
	}
}

func (t *Tunnel) listenRemote() {
	// Listen on remote server port
	listener, err := t.sshConn.Client.Listen("tcp", t.remoteEndpoint.String())
	if err != nil {
		log.Printf("[TUN] listen open port ON remote server error. %s\n", err)
		return
	}
	t.listener = listener

	log.Printf("[TUN] reverse connected. Local: %s -> Remote: %s\n", t.localEndpoint.String(), t.remoteEndpoint.String())
	if t.sshConn != nil && listener != nil {
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
			t.serveClient(client, local)
		}
		listener.Close()
	}
}

func (t *Tunnel) serveClient(client net.Conn, remote net.Conn) {
	var once sync.Once
	close := func() {
		client.Close()
		remote.Close()
	}

	// Start remote -> local data transfer
	go func() {
		io.Copy(client, remote)
		once.Do(close)

	}()

	// Start local -> remote data transfer
	go func() {
		io.Copy(remote, client)
		once.Do(close)
	}()
}
