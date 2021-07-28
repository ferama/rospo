package tun

import (
	"fmt"
	"log"
	"net"
	"sync"
	"time"

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
	stoppable bool

	registryID int

	clientsMap   map[string]net.Conn
	clientsMapMU sync.Mutex
}

// NewTunnel builds a Tunnel object
func NewTunnel(sshConn *sshc.SshConnection, conf *TunnelConf, stoppable bool) *Tunnel {

	tunnel := &Tunnel{
		forward:        conf.Forward,
		remoteEndpoint: conf.GetRemotEndpoint(),
		localEndpoint:  conf.GetLocalEndpoint(),

		sshConn:              sshConn,
		reconnectionInterval: 5 * time.Second,
		terminate:            make(chan bool, 1),
		stoppable:            stoppable,

		clientsMap: make(map[string]net.Conn),
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
	t.registryID = TunRegistry().Add(t)

	for {
		// waits for the ssh client to be connected to the server or for
		// a terminate request
		log.Println("[TUN] wait for ssh client to be ready")
		for {
			if t.waitForSshClient() {
				break
			} else {
				return
			}
		}
		log.Println("[TUN] ssh client ready")

		if t.forward {
			t.listenLocal()
		} else {
			t.listenRemote()
		}

		time.Sleep(t.reconnectionInterval)
	}
}

func (t *Tunnel) IsStoppable() bool {
	return t.stoppable
}

// Stop ends the tunnel
func (t *Tunnel) Stop() {
	if !t.stoppable {
		return
	}

	TunRegistry().Delete(t.registryID)
	close(t.terminate)
	go func() {
		if t.listener != nil {
			t.listener.Close()
		}

		// close all clients connections
		t.clientsMapMU.Lock()
		for k, v := range t.clientsMap {
			v.Close()
			delete(t.clientsMap, k)
		}
		t.clientsMapMU.Unlock()
	}()
}

func (t *Tunnel) listenLocal() error {
	// Listen on remote server port
	listener, err := net.Listen("tcp", t.localEndpoint.String())
	if err != nil {
		log.Printf("[TUN] dial INTO remote service error. %s\n", err)
		return err
	}
	t.listener = listener
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
			t.clientsMapMU.Lock()
			t.clientsMap[client.RemoteAddr().String()] = client
			t.clientsMapMU.Unlock()

			utils.CopyConnWithOnClose(client, remote, func() {
				t.clientsMapMU.Lock()
				delete(t.clientsMap, client.RemoteAddr().String())
				t.clientsMapMU.Unlock()
			})
		}
		listener.Close()
	}
	return nil
}

// GetListenerAddr returns the tunnel listener network address
func (t *Tunnel) GetListenerAddr() net.Addr {
	if t.listener != nil {
		return t.listener.Addr()
	}
	return nil
}

// GetActiveClientsCount returns how many clients are actually using the tunnel
func (t *Tunnel) GetActiveClientsCount() int {
	return len(t.clientsMap)
}

func (t *Tunnel) GetIsListenerLocal() bool {
	return t.forward
}

func (t *Tunnel) GetEndpoint() utils.Endpoint {
	if t.forward {
		return *t.remoteEndpoint
	} else {
		return *t.localEndpoint
	}
}

func (t *Tunnel) listenRemote() error {
	// Listen on remote server port
	// you can use port :0 to get a radnom available tcp port
	// Example:
	//	listener, err := t.sshConn.Client.Listen("tcp", "127.0.0.1:0")

	listener, err := t.sshConn.Client.Listen("tcp", t.remoteEndpoint.String())

	if err != nil {
		log.Printf("[TUN] listen open port ON remote server error. %s\n", err)
		return err
	}
	t.listener = listener

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

			t.clientsMapMU.Lock()
			t.clientsMap[client.RemoteAddr().String()] = client
			t.clientsMapMU.Unlock()

			utils.CopyConnWithOnClose(client, local, func() {
				t.clientsMapMU.Lock()
				delete(t.clientsMap, client.RemoteAddr().String())
				t.clientsMapMU.Unlock()
			})
		}
		listener.Close()
	}
	return nil
}
