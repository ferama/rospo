package sshc

import (
	"fmt"
	"io"
	"net"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"testing"
	"time"

	"github.com/ferama/rospo/pkg/sshd"
	"golang.org/x/net/proxy"
)

func getPort(addr net.Addr) string {
	parts := strings.Split(addr.String(), ":")
	return parts[1]
}

func startD(withPass bool, disableShell bool) string {
	serverConf := &sshd.SshDConf{
		Key:           "../../testdata/server",
		ListenAddress: "127.0.0.1:0",
		DisableShell:  disableShell,
	}
	if !withPass {
		serverConf.AuthorizedKeysURI = []string{"../../testdata/authorized_keys"}
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
		Identity:  "../../testdata/client",
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
	sshd1Port := startD(false, false)
	clientConf = &SshClientConf{
		Identity: "testdata/client",
		Insecure: true, // disables known_hosts check
		JumpHosts: []*JumpHostConf{
			{
				URI:      fmt.Sprintf("127.0.0.1:%s", "48739"),
				Identity: "../../testdata/client",
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
	sshdPort := startD(false, false)

	file, err := os.CreateTemp("", "rospo_known_hosts")
	if err != nil {
		log.Fatal(err)
	}
	defer os.Remove(file.Name())

	// create an ssh client
	clientConf := &SshClientConf{
		Identity:   "../../testdata/client",
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
	sshd1Port := startD(false, false)
	sshd2Port := startD(false, false)
	sshd3Port := startD(false, false)

	// create an ssh client
	clientConf := &SshClientConf{
		Identity: "../../testdata/client",
		Insecure: true, // disables known_hosts check
		JumpHosts: []*JumpHostConf{
			{
				URI:      fmt.Sprintf("127.0.0.1:%s", sshd2Port),
				Identity: "../../testdata/client",
			},
			{
				URI:      fmt.Sprintf("127.0.0.1:%s", sshd3Port),
				Identity: "../../testdata/client",
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
	sshdPort := startD(true, false)
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
	sshdPort := startD(false, false)
	clientConf := &SshClientConf{
		ServerURI: fmt.Sprintf("127.0.0.1:%s", sshdPort),
		Identity:  "../../testdata/client",
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
	sshdPort := startD(false, false)
	clientConf := &SshClientConf{
		ServerURI: fmt.Sprintf("127.0.0.1:%s", sshdPort),
		Identity:  "../../testdata/client",
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

func TestShellDisabled(t *testing.T) {
	sshdPort := startD(false, true)
	clientConf := &SshClientConf{
		ServerURI: fmt.Sprintf("127.0.0.1:%s", sshdPort),
		Identity:  "../../testdata/client",
		JumpHosts: make([]*JumpHostConf, 0),
		Insecure:  true,
	}
	client := NewSshConnection(clientConf)
	go client.Start()
	client.Connected.Wait()
	remoteShell := NewRemoteShell(client)
	err := remoteShell.Start("ls", false)
	if err == nil {
		t.Fatalf("shell/exec disabled. test should fail")
	}
	remoteShell.Stop()
	client.Stop()
}

func TestSocksProxy(t *testing.T) {
	sshdPort := startD(false, false)
	clientConf := &SshClientConf{
		ServerURI: fmt.Sprintf("127.0.0.1:%s", sshdPort),
		Identity:  "../../testdata/client",
		JumpHosts: make([]*JumpHostConf, 0),
		Insecure:  true,
	}
	client := NewSshConnection(clientConf)
	go client.Start()
	defer client.Stop()

	sockProxy := NewSocksProxy(client)
	go sockProxy.Start("127.0.0.1:10800")

	time.Sleep(2 * time.Second)

	const testResponse = "socks-test"
	httpServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		fmt.Fprint(w, testResponse)
	}))
	defer httpServer.Close()

	t.Log("starting socks client...")
	socksClient, err := proxy.SOCKS5("tcp", "127.0.0.1:10800", nil, proxy.Direct)
	if err != nil {
		t.Fatal(err)
	}
	tr := &http.Transport{Dial: socksClient.Dial}

	// Create client
	httpClient := &http.Client{
		Transport: tr,
	}

	t.Log("do http req...")
	resp, err := httpClient.Get(httpServer.URL)
	if err != nil {
		t.Fatal(err)
	}
	bytes, err := io.ReadAll(resp.Body)
	if err != nil {
		t.Fatal(err)
	}
	if string(bytes) != testResponse {
		t.Logf("expected: %s, have: %s", testResponse, string(bytes))
		t.Fail()
	}
}
