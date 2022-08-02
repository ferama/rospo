package sshd

import (
	"bufio"
	"errors"
	"fmt"
	"net"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/ferama/rospo/pkg/sshc"
	"github.com/ferama/rospo/pkg/tun"
	"github.com/pkg/sftp"
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

func TestNoIdentity(t *testing.T) {
	tmpDir := t.TempDir()

	tmpPath := filepath.Join(tmpDir, "notexisting")
	// start a local sshd
	serverConf := &SshDConf{
		Key:               tmpPath,
		AuthorizedKeysURI: []string{tmpPath},
		ListenAddress:     "127.0.0.1:0",
		DisableShell:      false,
		DisableAuth:       true,
	}
	sd := NewSshServer(serverConf)
	defer func() {
		os.Remove(tmpPath)
		os.Remove(filepath.Join(tmpDir, "notexisting.pub"))
	}()
	go sd.Start()
	var addr net.Addr
	for {
		addr = sd.GetListenerAddr()
		if addr != nil {
			break
		}
		time.Sleep(500 * time.Millisecond)
	}
	if _, err := os.Stat(tmpPath); errors.Is(err, os.ErrNotExist) {
		t.Fatal(err)
	}
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

func TestTunnelReverse(t *testing.T) {
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

	client := sshc.NewSshConnection(clientConf)
	go client.Start()

	echoListener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fail()
	}
	defer echoListener.Close()
	go startEchoService(echoListener)

	echoPort := getPort(echoListener.Addr())
	tunnelConf := &tun.TunnelConf{
		Remote:  "127.0.0.1:0",
		Local:   "127.0.0.1:" + echoPort,
		Forward: false,
	}
	tunnel := tun.NewTunnel(client, tunnelConf, true)
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

	tunnel.Stop()
}

func TestTunnelForward(t *testing.T) {
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

	client := sshc.NewSshConnection(clientConf)
	go client.Start()

	echoListener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fail()
	}
	defer echoListener.Close()
	go startEchoService(echoListener)

	echoPort := getPort(echoListener.Addr())
	tunnelConf := &tun.TunnelConf{
		Remote:  "127.0.0.1:" + echoPort,
		Local:   "127.0.0.1:0",
		Forward: true,
	}
	tunnel := tun.NewTunnel(client, tunnelConf, true)
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

	tunnel.Stop()
}

func TestRemoteExecute(t *testing.T) {
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
	session, err := conn.Client.NewSession()
	if err != nil {
		t.Fail()
	}

	if err := session.Run("some_not_existing_binary"); err != nil {
		if err.Error() != "Process exited with status 127" {
			t.Fail()
		}
	}
}
