package sshc

import (
	"context"
	"net"

	"github.com/ferama/go-socks"
)

type SocksProxy struct {
	sshConn *SshConnection
}

func NewSocksProxy(sshConn *SshConnection) *SocksProxy {
	p := &SocksProxy{
		sshConn: sshConn,
	}

	return p
}

// Start starts the local socks proxy
func (p *SocksProxy) Start(socksAddress string) error {
	p.sshConn.Connected.Wait()

	server, _ := socks.New(&socks.Config{
		Logger: log,
		Dial: func(ctx context.Context, network, addr string) (net.Conn, error) {
			return p.sshConn.Client.Dial(network, addr)
		},
	})

	log.Printf("local socks proxy listening at '%s'", socksAddress)
	if err := server.ListenAndServe("tcp", socksAddress); err != nil {
		return err
	}
	return nil
}
