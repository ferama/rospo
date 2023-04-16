package sshc

import (
	"context"
	"net"

	"github.com/armon/go-socks5"
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

	conf := &socks5.Config{
		Logger: log,
		Dial: func(ctx context.Context, network, addr string) (net.Conn, error) {
			return p.sshConn.Client.Dial(network, addr)
		},
	}

	serverSocks, err := socks5.New(conf)
	if err != nil {
		return err
	}

	log.Printf("local sock5 proxy listening at '%s'", socks5Address)
	if err := serverSocks.ListenAndServe("tcp", socks5Address); err != nil {
		return err
	}
	return nil
}
