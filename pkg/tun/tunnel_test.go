package tun

import (
	"bufio"
	"fmt"
	"net"
	"strings"
	"testing"
	"time"

	"github.com/ferama/rospo/pkg/sshc"
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

func getPort(addr net.Addr) string {
	parts := strings.Split(addr.String(), ":")
	return parts[1]
}

func TestTunnelReverse(t *testing.T) {
	// start a local sshd
	serverConf := &sshd.SshDConf{
		Key:               "testdata/server",
		AuthorizedKeysURI: []string{"testdata/authorized_keys"},
		ListenAddress:     "127.0.0.1:0",
		DisableShell:      false,
	}
	sd := sshd.NewSshServer(serverConf)
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
		Identity:  "testdata/client",
		Insecure:  true, // disable known_hosts check
		JumpHosts: make([]*sshc.JumpHostConf, 0),
		ServerURI: fmt.Sprintf("127.0.0.1:%s", sshdPort),
	}

	client := sshc.NewSshConnection(clientConf)
	go client.Start()

	echoListener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fail()
	}
	defer echoListener.Close()
	go startEchoService(echoListener)

	echoPort := getPort(echoListener.Addr())
	tunnelConf := &TunnelConf{
		Remote:  "127.0.0.1:0",
		Local:   "127.0.0.1:" + echoPort,
		Forward: false,
	}
	tunnel := NewTunnel(client, tunnelConf, true)
	go tunnel.Start()

	var tunaddr net.Addr
	for {
		tunaddr = tunnel.GetListenerAddr()
		if tunaddr != nil {
			break
		}
		time.Sleep(500 * time.Millisecond)
	}
	t.Log(tunaddr)

	conn, err := net.Dial("tcp", tunaddr.String())
	if err != nil {
		t.Error(err)
	}
	_, err = conn.Write([]byte("test\n"))
	if err != nil {
		t.Error(err)
	}
	buf := make([]byte, 4)
	_, err = conn.Read(buf)
	if err != nil {
		t.Error(err)
	}
	if string(buf) != "test" {
		t.Error("assert data written is equal to data read")
	}

	if !tunnel.IsStoppable() {
		t.Fail()
	}
	if !(tunnel.GetActiveClientsCount() == 1) {
		t.Fail()
	}

	tunnel.GetIsListenerLocal()
	tunnel.GetEndpoint()

	tunnel.Stop()
	// be sure to catch the full stop event
	time.Sleep(tunnel.reconnectionInterval + 2*time.Second)
}

func TestTunnelForward(t *testing.T) {
	// start a local sshd
	serverConf := &sshd.SshDConf{
		Key:               "testdata/server",
		AuthorizedKeysURI: []string{"testdata/authorized_keys"},
		ListenAddress:     "127.0.0.1:0",
		DisableShell:      false,
	}
	sd := sshd.NewSshServer(serverConf)
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
		Identity:  "testdata/client",
		Insecure:  true, // disable known_hosts check
		JumpHosts: make([]*sshc.JumpHostConf, 0),
		ServerURI: fmt.Sprintf("127.0.0.1:%s", sshdPort),
	}

	client := sshc.NewSshConnection(clientConf)
	go client.Start()

	echoListener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fail()
	}
	defer echoListener.Close()
	go startEchoService(echoListener)

	echoPort := getPort(echoListener.Addr())
	tunnelConf := &TunnelConf{
		Remote:  "127.0.0.1:" + echoPort,
		Local:   "127.0.0.1:0",
		Forward: true,
	}
	tunnel := NewTunnel(client, tunnelConf, true)
	go tunnel.Start()

	var tunaddr net.Addr
	for {
		tunaddr = tunnel.GetListenerAddr()
		if tunaddr != nil {
			break
		}
		time.Sleep(500 * time.Millisecond)
	}

	t.Log(tunaddr)
	conn, err := net.Dial("tcp", tunaddr.String())
	if err != nil {
		t.Error(err)
	}
	_, err = conn.Write([]byte("test\n"))
	if err != nil {
		t.Error(err)
	}
	buf := make([]byte, 4)
	_, err = conn.Read(buf)
	if err != nil {
		t.Error(err)
	}
	if string(buf) != "test" {
		t.Error("assert data written is equal to data read")
	}

	if !tunnel.IsStoppable() {
		t.Fail()
	}
	if !(tunnel.GetActiveClientsCount() == 1) {
		t.Fail()
	}

	tunnel.GetIsListenerLocal()
	tunnel.GetEndpoint()

	tunnel.Stop()
}
