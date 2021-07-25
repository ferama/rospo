package pipe

import (
	"errors"
	"log"
	"net"

	"github.com/ferama/rospo/pkg/conf"
	"github.com/ferama/rospo/pkg/utils"
)

// Pipe struct definition
type Pipe struct {
	local  *utils.Endpoint
	remote *utils.Endpoint

	// the pipe connection listener
	listener net.Listener
}

// NewPipe creates a Pipe object
func NewPipe(conf *conf.PipeConf) *Pipe {
	return &Pipe{
		local:  utils.NewEndpoint(conf.Local),
		remote: utils.NewEndpoint(conf.Remote),
	}
}

// GetListenerAddr returns the pipe listener network address
func (p *Pipe) GetListenerAddr() (net.Addr, error) {
	if p.listener != nil {
		return p.listener.Addr(), nil
	} else {
		return &net.TCPAddr{}, errors.New("listener not ready")
	}
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
	log.Printf("[PIPE] listening on %s\n", p.local)
	for {
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
	listener.Close()
}

// Stop closes the pipe
func (p *Pipe) Stop() {
	if p.listener != nil {
		p.listener.Close()
	}
}
