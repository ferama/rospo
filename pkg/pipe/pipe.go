package pipe

import (
	"log"
	"net"

	"github.com/ferama/rospo/pkg/conf"
	"github.com/ferama/rospo/pkg/utils"
)

// Pipe struct definition
type Pipe struct {
	local  *utils.Endpoint
	remote *utils.Endpoint
}

// NewPipe creates a Pipe object
func NewPipe(conf *conf.PipeConf) *Pipe {
	return &Pipe{
		local:  utils.NewEndpoint(conf.Local),
		remote: utils.NewEndpoint(conf.Remote),
	}
}

// Start the pipe. It basically copy all the tcp packets incoming to the
// local endpoint into the remote endpoint
func (r *Pipe) Start() {
	listener, err := net.Listen("tcp", r.local.String())
	if err != nil {
		log.Printf("[PIPE] listening on %s error.\n", err)
		return
	}
	log.Printf("[PIPE] listening on %s\n", r.local)
	for {
		client, err := listener.Accept()
		if err != nil {
			log.Println("[PIPE] disconnected")
			break
		}
		go func() {
			conn, err := net.Dial("tcp", r.remote.String())
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
