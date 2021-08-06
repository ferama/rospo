package tun

import (
	"net"
	"sync"
	"time"

	"github.com/ferama/rospo/pkg/logger"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/ferama/rospo/pkg/utils"
)

var log = logger.NewLogger("[TUN]  ", logger.Magenta)

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

	listenerMU sync.RWMutex
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
		for {
			if t.waitForSshClient() {
				break
			} else {
				log.Println("terminated")
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

// IsStoppable return true if the tunnel can be stopped calling the Stop
// method. False if not
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
		t.listenerMU.RLock()
		if t.listener != nil {
			t.listener.Close()
		}
		t.listenerMU.RUnlock()

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
	log.Println("starting listen local")
	listener, err := net.Listen("tcp", t.localEndpoint.String())
	if err != nil {
		log.Printf("dial INTO remote service error. %s\n", err)
		return err
	}
	defer listener.Close()

	t.listenerMU.Lock()
	t.listener = listener
	t.listenerMU.Unlock()

	log.Printf("forward connected. Local: %s <- Remote: %s\n", t.listener.Addr(), t.remoteEndpoint.String())
	if t.sshConn != nil && listener != nil {
		for {
			remote, err := t.sshConn.Client.Dial("tcp", t.remoteEndpoint.String())
			// Open a (local) connection to localEndpoint whose content will be forwarded so serverEndpoint
			if err != nil {
				log.Printf("listen open port ON local server error. %s\n", err)
				break
			}
			client, err := listener.Accept()
			if err != nil {
				log.Println("disconnected")
				return err
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
	}
	return nil
}

// GetListenerAddr returns the tunnel listener network address
func (t *Tunnel) GetListenerAddr() net.Addr {
	t.listenerMU.RLock()
	defer t.listenerMU.RUnlock()

	if t.listener != nil {
		return t.listener.Addr()
	}
	return nil
}

// GetActiveClientsCount returns how many clients are actually using the tunnel
func (t *Tunnel) GetActiveClientsCount() int {
	t.clientsMapMU.Lock()
	defer t.clientsMapMU.Unlock()

	return len(t.clientsMap)
}

// GetIsListenerLocal return true if it is a forward tunnel. In a forward tunnel
// a socket listener is started on the local (rospo) machine and all the connections are forwarded
// to the remote endpoint. In a NON forward tunnel, the listener is started on the remote
// machine instead. When a client connects to the remote listener the connection is forwarded
// on the local endpoint
func (t *Tunnel) GetIsListenerLocal() bool {
	return t.forward
}

// GetEndpoint returns the tunnel endpoint
func (t *Tunnel) GetEndpoint() utils.Endpoint {
	if t.forward {
		return *t.remoteEndpoint
	}
	return *t.localEndpoint
}

func (t *Tunnel) listenRemote() error {
	// Listen on remote server port
	// you can use port :0 to get a random available tcp port
	// Example:
	//	listener, err := t.sshConn.Client.Listen("tcp", "127.0.0.1:0")
	log.Println("starting listen remote")
	listener, err := t.sshConn.Client.Listen("tcp", t.remoteEndpoint.String())
	if err != nil {
		log.Printf("listen open port ON remote server error. %s\n", err)
		return err
	}
	defer listener.Close()

	t.listenerMU.Lock()
	t.listener = listener
	t.listenerMU.Unlock()

	log.Printf("reverse connected. Local: %s -> Remote: %s\n", t.localEndpoint.String(), t.listener.Addr())
	if t.sshConn != nil && listener != nil {
		for {
			// Open a (local) connection to localEndpoint whose content will be forwarded so serverEndpoint
			local, err := net.Dial("tcp", t.localEndpoint.String())
			if err != nil {
				log.Printf("dial INTO local service error. %s\n", err)
				break
			}

			client, err := listener.Accept()
			if err != nil {
				log.Println("disconnected")
				return err
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
	}
	return nil
}
