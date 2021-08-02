package tun

import (
	"bufio"
	"net"
	"testing"

	"github.com/ferama/rospo/pkg/sshd"
)

func startEchoService(l net.Listener) {
	for {
		conn, err := l.Accept()
		if err != nil {
			continue
		}
		go func() {
			r := bufio.NewReader(conn)
			for {
				line, err := r.ReadBytes('\n')
				if err != nil {
					return
				}
				conn.Write(line)
			}
		}()
	}
}

func TestTunnel(t *testing.T) {
	// start a local sshd

	config := &sshd.SshDConf{
		Key:                "testdata/server",
		AuthorizedKeysFile: "testdata/authorized_keys",
		ListenAddress:      ":0",
		DisableShell:       false,
	}
	sd := sshd.NewSshServer(config)
	go sd.Start()
}
