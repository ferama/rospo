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
	listener   net.Listener
	listenerWg sync.WaitGroup

	// indicate if the pipe should be terminated
	terminate chan bool

	registryID int

	clientsMap   map[string]net.Conn
	clientsMapMU sync.Mutex
}

// NewPipe creates a Pipe object
func NewPipe(conf *PipeConf) *Pipe {
	pipe := &Pipe{
		local:      utils.NewEndpoint(conf.Local),
		remote:     utils.NewEndpoint(conf.Remote),
		terminate:  make(chan bool),
		clientsMap: make(map[string]net.Conn),
	}
	pipe.listenerWg.Add(1)
	return pipe
}

// GetListenerAddr returns the pipe listener network address
func (p *Pipe) GetListenerAddr() net.Addr {
	p.listenerWg.Wait()
	return p.listener.Addr()
}

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
	p.listenerWg.Done()

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

// Stop closes the pipe
func (p *Pipe) Stop() {
	PipeRegistry().Delete(p.registryID)
	close(p.terminate)
	go func() {
		p.listenerWg.Wait()
		p.listener.Close()

		// close all clients connections
		p.clientsMapMU.Lock()
		for k, v := range p.clientsMap {
			v.Close()
			delete(p.clientsMap, k)
		}
		p.clientsMapMU.Unlock()
	}()
}
