package forward

import (
	"io"
	"log"
	"net"
	"sync"

	"github.com/ferama/rospo/pkg/conf"
	"github.com/ferama/rospo/pkg/utils"
)

// Forward struct definition
type Forward struct {
	local  *utils.Endpoint
	remote *utils.Endpoint
}

// NewForward creates a Forward object
func NewForward(conf *conf.ForwardConf) *Forward {
	return &Forward{
		local:  utils.NewEndpoint(conf.Local),
		remote: utils.NewEndpoint(conf.Remote),
	}
}

// Start the forward. It basically copy all the tcp packets incoming to the
// local endpoint into the remote endpoint
func (r *Forward) Start() {
	listener, err := net.Listen("tcp", r.local.String())
	if err != nil {
		log.Printf("[ROUTER] listening on %s error.\n", err)
		return
	}
	log.Printf("[ROUTER] listening on %s\n", r.local)
	for {
		client, err := listener.Accept()
		if err != nil {
			log.Println("[ROUTER] disconnected")
			break
		}
		go func() {
			conn, err := net.Dial("tcp", r.remote.String())
			if err != nil {
				log.Println("[ROUTER] remote connection refused")
				client.Close()
				return
			}
			r.serveClient(client, conn)
		}()
	}
	listener.Close()
}

func (r *Forward) serveClient(local net.Conn, remote net.Conn) {
	var once sync.Once
	close := func() {
		local.Close()
		remote.Close()
	}

	// Start remote -> local data transfer
	go func() {
		io.Copy(local, remote)
		once.Do(close)

	}()

	// Start local -> remote data transfer
	go func() {
		io.Copy(remote, local)
		once.Do(close)
	}()
}
