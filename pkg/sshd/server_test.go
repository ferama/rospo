package sshd

import (
	"fmt"
	"net"
	"strings"
	"testing"
	"time"

	"github.com/ferama/rospo/pkg/sshc"
	"github.com/pkg/sftp"
)

func getPort(addr net.Addr) string {
	parts := strings.Split(addr.String(), ":")
	return parts[1]
}

func TestSftpSubsystem(t *testing.T) {
	// start a local sshd
	serverConf := &SshDConf{
		Key:               "../../testdata/server",
		AuthorizedKeysURI: []string{"../../testdata/authorized_keys"},
		ListenAddress:     "127.0.0.1:0",
		DisableShell:      false,
	}
	sd := NewSshServer(serverConf)
	go sd.Start()
	var addr net.Addr
	for {
		addr = sd.GetListenerAddr()
		if addr != nil {
			break
		}
		time.Sleep(500 * time.Millisecond)
	}
	sshdPort := getPort(addr)

	// create an ssh client
	clientConf := &sshc.SshClientConf{
		Identity:  "../../testdata/client",
		Insecure:  true, // disable known_hosts check
		JumpHosts: make([]*sshc.JumpHostConf, 0),
		ServerURI: fmt.Sprintf("127.0.0.1:%s", sshdPort),
	}

	conn := sshc.NewSshConnection(clientConf)
	go conn.Start()

	conn.Connected.Wait()

	client, err := sftp.NewClient(conn.Client)
	if err != nil {
		t.Error(err)
	}
	defer client.Close()
}
