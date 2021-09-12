package sshc

import (
	"fmt"
	"io/ioutil"
	"net"
	"os"
	"strings"
	"testing"
	"time"

	"github.com/ferama/rospo/pkg/sshd"
)

func getPort(addr net.Addr) string {
	parts := strings.Split(addr.String(), ":")
	return parts[1]
}

func startD(withPass bool) string {
	serverConf := &sshd.SshDConf{
		Key:           "testdata/server",
		ListenAddress: "127.0.0.1:0",
		DisableShell:  false,
	}
	if !withPass {
		serverConf.AuthorizedKeysURI = "testdata/authorized_keys"
	} else {
		serverConf.AuthorizedPassword = "password"
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
	return sshdPort
}

func TestErrors(t *testing.T) {
	// create an ssh client
	clientConf := &SshClientConf{
		Identity:  "testdata/client",
		Insecure:  true,
		JumpHosts: make([]*JumpHostConf, 0),
		ServerURI: fmt.Sprintf("127.0.0.1:%s", "48738"), // some random not existing port
	}
	client := NewSshConnection(clientConf)
	go client.Start()
	time.Sleep(2 * time.Second)
	if client.GetConnectionStatus() != STATUS_CONNECTING {
		t.Fail()
	}

	// invalid tunnel hop
	sshd1Port := startD(false)
	clientConf = &SshClientConf{
		Identity: "testdata/client",
		Insecure: true, // disables known_hosts check
		JumpHosts: []*JumpHostConf{
			{
				URI:      fmt.Sprintf("127.0.0.1:%s", "48739"),
				Identity: "testdata/client",
			},
		},
		ServerURI: fmt.Sprintf("127.0.0.1:%s", sshd1Port),
	}
	client = NewSshConnection(clientConf)
	go client.Start()
	time.Sleep(2 * time.Second)
	if client.GetConnectionStatus() != STATUS_CONNECTING {
		t.Fail()
	}
}

func TestSshC(t *testing.T) {
	sshdPort := startD(false)

	file, err := ioutil.TempFile("", "rospo_known_hosts")
	if err != nil {
		log.Fatal(err)
	}
	defer os.Remove(file.Name())

	// create an ssh client
	clientConf := &SshClientConf{
		Identity:   "testdata/client",
		KnownHosts: file.Name(),
		Insecure:   false,
		JumpHosts:  make([]*JumpHostConf, 0),
		ServerURI:  fmt.Sprintf("127.0.0.1:%s", sshdPort),
	}

	client := NewSshConnection(clientConf)
	client.GrabPubKey()
	go client.Start()

	client.Connected.Wait()
}

func TestJumpHosts(t *testing.T) {
	sshd1Port := startD(false)
	sshd2Port := startD(false)
	sshd3Port := startD(false)

	// create an ssh client
	clientConf := &SshClientConf{
		Identity: "testdata/client",
		Insecure: true, // disables known_hosts check
		JumpHosts: []*JumpHostConf{
			{
				URI:      fmt.Sprintf("127.0.0.1:%s", sshd2Port),
				Identity: "testdata/client",
			},
			{
				URI:      fmt.Sprintf("127.0.0.1:%s", sshd3Port),
				Identity: "testdata/client",
			},
		},
		ServerURI: fmt.Sprintf("127.0.0.1:%s", sshd1Port),
	}
	client := NewSshConnection(clientConf)
	go client.Start()
	client.Connected.Wait()
	client.Stop()
}

func TestWithPassword(t *testing.T) {
	sshdPort := startD(true)
	clientConf := &SshClientConf{
		ServerURI: fmt.Sprintf("127.0.0.1:%s", sshdPort),
		JumpHosts: make([]*JumpHostConf, 0),
		Insecure:  true,
		Password:  "password",
	}
	client := NewSshConnection(clientConf)
	go client.Start()
	client.Connected.Wait()
	client.Stop()
}

func TestRemoteShell(t *testing.T) {
	sshdPort := startD(false)
	clientConf := &SshClientConf{
		ServerURI: fmt.Sprintf("127.0.0.1:%s", sshdPort),
		Identity:  "testdata/client",
		JumpHosts: make([]*JumpHostConf, 0),
		Insecure:  true,
	}
	client := NewSshConnection(clientConf)
	go client.Start()
	remoteShell := NewRemoteShell(client)
	go remoteShell.Start("", true)
	time.Sleep(1 * time.Second)
	remoteShell.Stop()
	client.Stop()
}

func TestRemoteShellCmd(t *testing.T) {
	sshdPort := startD(false)
	clientConf := &SshClientConf{
		ServerURI: fmt.Sprintf("127.0.0.1:%s", sshdPort),
		Identity:  "testdata/client",
		JumpHosts: make([]*JumpHostConf, 0),
		Insecure:  true,
	}
	client := NewSshConnection(clientConf)
	go client.Start()
	remoteShell := NewRemoteShell(client)
	go remoteShell.Start("ls", false)
	time.Sleep(1 * time.Second)
	remoteShell.Stop()
	client.Stop()
}
