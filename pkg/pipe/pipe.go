package pipe

import (
	"log"
	"net"
	"sync"

	"github.com/ferama/rospo/pkg/utils"
)

// Pipe struct definition
type Pipe struct {
	local  *utils.Endpoint
	remote *utils.Endpoint

	// the pipe connection listener
	listener net.Listener

	// indicate if the pipe should be terminated
	terminate chan bool
	stoppable bool

	registryID int

	clientsMap   map[string]net.Conn
	clientsMapMU sync.Mutex
}

// NewPipe creates a Pipe object
func NewPipe(conf *PipeConf, stoppable bool) *Pipe {
	pipe := &Pipe{
		local:      utils.NewEndpoint(conf.Local),
		remote:     utils.NewEndpoint(conf.Remote),
		terminate:  make(chan bool),
		stoppable:  stoppable,
		clientsMap: make(map[string]net.Conn),
	}
	return pipe
}

// GetListenerAddr returns the pipe listener network address
func (p *Pipe) GetListenerAddr() net.Addr {
	if p.listener != nil {
		return p.listener.Addr()
	}
	return nil
}

// GetEndpoint returns the pipe remote endpoint
// This is actually used from pipe api routes
func (p *Pipe) GetEndpoint() utils.Endpoint {
	return *p.remote
}

// GetActiveClientsCount returns how many clients are actually using the pipe
func (p *Pipe) GetActiveClientsCount() int {
	return len(p.clientsMap)
}

// Start the pipe. It basically copy all the tcp packets incoming to the
// local endpoint into the remote endpoint
func (p *Pipe) Start() {
	listener, err := net.Listen("tcp", p.local.String())
	p.listener = listener

	if err != nil {
		log.Printf("[PIPE] listening on %s error.\n", err)
		return
	}
	p.registryID = PipeRegistry().Add(p)

	log.Printf("[PIPE] listening on %s\n", p.local)
	for {
		select {
		case <-p.terminate:
			return
		default:
			client, err := listener.Accept()
			if err != nil {
				log.Println("[PIPE] disconnected")
				break
			}
			p.clientsMapMU.Lock()
			p.clientsMap[client.RemoteAddr().String()] = client
			p.clientsMapMU.Unlock()
			go func() {
				conn, err := net.Dial("tcp", p.remote.String())
				if err != nil {
					log.Println("[PIPE] remote connection refused")
					client.Close()
					return
				}
				utils.CopyConnWithOnClose(client, conn, func() {
					p.clientsMapMU.Lock()
					delete(p.clientsMap, client.RemoteAddr().String())
					p.clientsMapMU.Unlock()
				})
			}()
		}

	}
}

// IsStoppable return true if the pipe can be stopped calling the Stop
// method. False if not
func (p *Pipe) IsStoppable() bool {
	return p.stoppable
}

// Stop closes the pipe
func (p *Pipe) Stop() {
	if !p.stoppable {
		return
	}
	PipeRegistry().Delete(p.registryID)
	close(p.terminate)
	go func() {
		// if p.listener != nil {
		// 	p.listener.Close()
		// }

		// close all clients connections
		p.clientsMapMU.Lock()
		for k, v := range p.clientsMap {
			v.Close()
			delete(p.clientsMap, k)
		}
		p.clientsMapMU.Unlock()
	}()
}
