package tun

import (
	"errors"
	"fmt"
	"log"
	"net"
	"sync"
	"time"

	"github.com/ferama/rospo/pkg/conf"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/ferama/rospo/pkg/utils"
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

	// indicate if the tunnel should be terminated
	terminate chan bool

	listenerWg sync.WaitGroup
}

// NewTunnel builds a Tunnel object
func NewTunnel(sshConn *sshc.SshConnection, conf *conf.TunnelConf) *Tunnel {

	tunnel := &Tunnel{
		forward:        conf.Forward,
		remoteEndpoint: conf.GetRemotEndpoint(),
		localEndpoint:  conf.GetLocalEndpoint(),

		sshConn:              sshConn,
		reconnectionInterval: 5 * time.Second,
		terminate:            make(chan bool, 1),
	}

	return tunnel
}

func (t *Tunnel) waitForSshClient() bool {
	c := make(chan bool)
	go func() {
		defer close(c)
		// WARN: if I have issues with sshConn this will wait forever
		t.sshConn.Connected.Wait()
	}()
	select {
	case <-t.terminate:
		return false
	default:
		select {
		case <-c:
			return true
		case <-t.terminate:
			return false
		}
	}
}

// Start activates the tunnel connections
func (t *Tunnel) Start() {
	for {
		t.listenerWg.Add(1)
		// waits for the ssh client to be connected to the server or for
		// a terminate request
		for {
			if t.waitForSshClient() {
				break
			} else {
				return
			}
		}

		if t.forward {
			t.listenLocal()
		} else {
			t.listenRemote()
		}

		time.Sleep(t.reconnectionInterval)
	}
}

// Stop ends the tunnel
func (t *Tunnel) Stop() {
	close(t.terminate)
	go func() {
		t.listenerWg.Wait()
		t.listener.Close()
	}()
}

func (t *Tunnel) listenLocal() {
	// Listen on remote server port
	listener, err := net.Listen("tcp", t.localEndpoint.String())
	if err != nil {
		log.Printf("[TUN] dial INTO remote service error. %s\n", err)
		return
	}
	t.listener = listener
	t.listenerWg.Done()

	log.Printf("[TUN] forward connected. Local: %s <- Remote: %s\n", t.listener.Addr(), t.remoteEndpoint.String())
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
			utils.CopyConn(client, remote)
		}
		listener.Close()
	}
}

// GetListenerAddr returns the tunnel listener netowork address
func (t *Tunnel) GetListenerAddr() (net.Addr, error) {
	if t.listener != nil {
		return t.listener.Addr(), nil
	} else {
		return &net.TCPAddr{}, errors.New("listener not ready")
	}
}

func (t *Tunnel) listenRemote() {
	// Listen on remote server port
	// you can use port :0 to get a radnom available tcp port
	// Example:
	//	listener, err := t.sshConn.Client.Listen("tcp", "127.0.0.1:0")

	listener, err := t.sshConn.Client.Listen("tcp", t.remoteEndpoint.String())

	if err != nil {
		log.Printf("[TUN] listen open port ON remote server error. %s\n", err)
		return
	}
	t.listener = listener
	t.listenerWg.Done()
	log.Printf("[TUN] reverse connected. Local: %s -> Remote: %s\n", t.localEndpoint.String(), t.listener.Addr())
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
			utils.CopyConn(client, local)
		}
		listener.Close()
	}
}
