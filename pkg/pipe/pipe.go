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
}

// NewPipe creates a Pipe object
func NewPipe(conf *PipeConf) *Pipe {
	pipe := &Pipe{
		local:     utils.NewEndpoint(conf.Local),
		remote:    utils.NewEndpoint(conf.Remote),
		terminate: make(chan bool),
	}
	pipe.listenerWg.Add(1)
	return pipe
}

// GetListenerAddr returns the pipe listener network address
func (p *Pipe) GetListenerAddr() net.Addr {
	p.listenerWg.Wait()
	return p.listener.Addr()
}

// Start the pipe. It basically copy all the tcp packets incoming to the
// local endpoint into the remote endpoint
func (p *Pipe) Start() {
	p.registryID = PipeRegistry().Add(p)

	listener, err := net.Listen("tcp", p.local.String())
	p.listener = listener
	p.listenerWg.Done()

	if err != nil {
		log.Printf("[PIPE] listening on %s error.\n", err)
		return
	}
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
			go func() {
				conn, err := net.Dial("tcp", p.remote.String())
				if err != nil {
					log.Println("[PIPE] remote connection refused")
					client.Close()
					return
				}
				utils.CopyConn(client, conn)
			}()
		}

	}
	// listener.Close()
}

// Stop closes the pipe
func (p *Pipe) Stop() {
	PipeRegistry().Delete(p.registryID)
	close(p.terminate)
	go func() {
		p.listenerWg.Wait()
		p.listener.Close()
	}()
}
