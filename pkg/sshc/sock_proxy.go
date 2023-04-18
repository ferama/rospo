package sshc

import (
	"context"
	"net"

	"github.com/things-go/go-socks5"
)

type SocksProxy struct {
	sshConn *SshConnection
}

func NewSockProxy(sshConn *SshConnection) *SocksProxy {
	p := &SocksProxy{
		sshConn: sshConn,
	}

	return p
}

// Start starts the local socks proxy
func (p *SocksProxy) Start(socks5Address string) error {
	p.sshConn.Connected.Wait()

	server := socks5.NewServer(
		socks5.WithLogger(socks5.NewLogger(log)),
		socks5.WithDial(func(ctx context.Context, network, addr string) (net.Conn, error) {
			return p.sshConn.Client.Dial(network, addr)
		}),
	)

	log.Printf("local sock5 proxy listening at '%s'", socks5Address)
	if err := server.ListenAndServe("tcp", socks5Address); err != nil {
		return err
	}
	return nil
}
