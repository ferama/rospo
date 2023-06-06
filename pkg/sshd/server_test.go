package sshd

import (
	"fmt"
	"math/rand"
	"net"
	"strings"
	"testing"
	"time"

	"github.com/ferama/rospo/pkg/sshc"
	"golang.org/x/crypto/ssh"
)

func getPort(addr net.Addr) string {
	parts := strings.Split(addr.String(), ":")
	return parts[1]
}

func startD(disableSftp bool) (*sshServer, string) {
	serverConf := &SshDConf{
		Key:                  "../../testdata/server",
		ListenAddress:        "127.0.0.1:0",
		DisableSftpSubsystem: disableSftp,
	}
	serverConf.AuthorizedKeysURI = []string{"../../testdata/authorized_keys"}
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
	return sd, sshdPort
}

func getSSHConn(sshdPort string) *sshc.SshConnection {
	// create an ssh client
	clientConf := &sshc.SshClientConf{
		Identity:  "../../testdata/client",
		Insecure:  true, // disable known_hosts check
		JumpHosts: make([]*sshc.JumpHostConf, 0),
		ServerURI: fmt.Sprintf("127.0.0.1:%s", sshdPort),
	}

	client := sshc.NewSshConnection(clientConf)
	go client.Start()
	client.ReadyWait()

	return client
}

func generateClients(n int, sshdPort string) {
	for i := 0; i < n; i++ {
		getSSHConn(sshdPort)
	}
}

func TestActiveSessions(t *testing.T) {
	sd, sshdPort := startD(false)

	nClients := rand.Intn(10) + 1
	generateClients(nClients, sshdPort)
	if sd.GetActiveSessionsCount() != nClients {
		t.Fatalf("has '%d' sessions, expected '%d", sd.GetActiveSessionsCount(), nClients)
	}

	t.Logf("==== generated '%d' clients", nClients)
}

func TestSftpEnabled(t *testing.T) {
	_, sshdPort := startD(false)
	conn := getSSHConn(sshdPort)
	var payload = struct{ Name string }{}
	payload.Name = "sftp"
	ch, _, err := conn.Client.OpenChannel("session", nil)
	if err != nil {
		t.Error(err)
	}
	ok, err := ch.SendRequest("subsystem", true, ssh.Marshal(payload))
	if err != nil {
		t.Error(err)
	}
	if !ok {
		t.Fatal("sftp subsystem expected")
	}
}

func TestSftpDisabled(t *testing.T) {
	_, sshdPort := startD(true)
	conn := getSSHConn(sshdPort)

	sess, _ := conn.Client.NewSession()
	err := sess.RequestSubsystem("sftp")
	if err == nil {
		t.Fatal("expected sftp subsystem to be disabled")
	}
}
