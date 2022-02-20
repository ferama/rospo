package utils

import (
	"fmt"
	"log"
	"net"
	"os/user"
	"sync"
	"testing"
)

func TestSSHUrlParser(t *testing.T) {
	compare := func(s1 *sshUrl, s2 *sshUrl) bool {
		if s1.Host != s2.Host ||
			s1.Port != s2.Port ||
			s1.Username != s2.Username {
			return false
		}
		return true
	}

	currentUser, _ := user.Current()

	list := []string{
		"user@192.168.0.1:22",
		"192.168.0.1",
		"192.168.0.1:2222",
		":22",
		"user-name@192.168.0.1:2222",
		"user@dm1.dm2.dm3.com",
		"user@dm1.dm2.dm3.com:2222",
	}

	expected := []sshUrl{
		{Username: "user", Host: "192.168.0.1", Port: 22},
		{Username: currentUser.Username, Host: "192.168.0.1", Port: 22},
		{Username: currentUser.Username, Host: "192.168.0.1", Port: 2222},
		{Username: currentUser.Username, Host: "127.0.0.1", Port: 22},
		{Username: "user-name", Host: "192.168.0.1", Port: 2222},
		{Username: "user", Host: "dm1.dm2.dm3.com", Port: 22},
		{Username: "user", Host: "dm1.dm2.dm3.com", Port: 2222},
	}
	for idx, s := range list {
		parsed := ParseSSHUrl(s)
		if !compare(parsed, &expected[idx]) {
			t.Fatalf("+%v", &expected[idx])
		}
	}
}

func TestExpandHome(t *testing.T) {
	_, err := ExpandUserHome("~/.ssh")
	if err != nil {
		t.Fail()
	}
	_, err = ExpandUserHome("/app/.ssh")
	if err != nil {
		t.Fail()
	}
}

func TestDefaultShell(t *testing.T) {
	shell := GetUserDefaultShell("notexistsinguser")
	if shell != "/bin/sh" {
		t.Fail()
	}
}

func TestCopyConn(t *testing.T) {
	var c1WG sync.WaitGroup
	var c2WG sync.WaitGroup
	var port1 string
	var port2 string
	const payload = "test"

	c1WG.Add(1)
	c2WG.Add(1)

	go func() {
		remote, err := net.Listen("tcp", "127.0.0.1:0")
		if err != nil {
			log.Fatal(err)
		}
		_, port1, _ = net.SplitHostPort(remote.Addr().String())
		c1WG.Done()

		for {
			conn, err := remote.Accept()
			if err != nil {
				log.Fatal(err)
			}

			go func(net.Conn) {
				conn.Write([]byte(payload))
				conn.Close()
			}(conn)
		}
	}()

	go func() {
		c1WG.Wait()

		listen, err := net.Listen("tcp", "127.0.0.1:0")
		if err != nil {
			log.Fatal(err)
		}
		_, port2, _ = net.SplitHostPort(listen.Addr().String())
		c2WG.Done()
		for {
			client, err := listen.Accept()
			if err != nil {
				log.Fatal(err)
			}
			go func() {
				conn, _ := net.Dial("tcp", fmt.Sprintf("127.0.0.1:%s", port1))
				CopyConn(conn, client, nil)
			}()
		}
	}()

	c1WG.Wait()
	c2WG.Wait()

	conn, _ := net.Dial("tcp", fmt.Sprintf("127.0.0.1:%s", port2))
	buf := make([]byte, len(payload))
	conn.Read(buf)
	if string(buf) != payload {
		t.Fail()
	}
}
