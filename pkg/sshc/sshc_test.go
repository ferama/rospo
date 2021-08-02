package sshc

import (
	"fmt"
	"io/ioutil"
	"log"
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

func TestSshC(t *testing.T) {
	serverConf := &sshd.SshDConf{
		Key:                "testdata/server",
		AuthorizedKeysFile: "testdata/authorized_keys",
		ListenAddress:      "127.0.0.1:0",
		DisableShell:       false,
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
