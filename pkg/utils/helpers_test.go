package utils

import (
	"log"
	"os/user"
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
		"user@[2001:0db8:85a3:0000:0000:8a2e:0370:7334]:2222",
		"192.168.0.1",
		"192.168.0.1:2222",
		":22",
		"user-name@192.168.0.1:2222",
		"user@dm1.dm2.dm3.com",
		"user@dm1.dm2.dm3.com:2222",
		"[2001:0db8:85a3:0000:0000:8a2e:0370:7334]",
		"[2001:0db8:85a3:0000:0000:8a2e:0370:7334]:2222",
		"user@[2001:0db8:85a3:0000:0000:8a2e:0370:7334]:2222",
	}

	expected := []sshUrl{
		{Username: "user", Host: "192.168.0.1", Port: 22},
		{Username: "user", Host: "[2001:0db8:85a3:0000:0000:8a2e:0370:7334]", Port: 2222},
		{Username: currentUser.Username, Host: "192.168.0.1", Port: 22},
		{Username: currentUser.Username, Host: "192.168.0.1", Port: 2222},
		{Username: currentUser.Username, Host: "127.0.0.1", Port: 22},
		{Username: "user-name", Host: "192.168.0.1", Port: 2222},
		{Username: "user", Host: "dm1.dm2.dm3.com", Port: 22},
		{Username: "user", Host: "dm1.dm2.dm3.com", Port: 2222},
		{Username: currentUser.Username, Host: "[2001:0db8:85a3:0000:0000:8a2e:0370:7334]", Port: 22},
		{Username: currentUser.Username, Host: "[2001:0db8:85a3:0000:0000:8a2e:0370:7334]", Port: 2222},
		{Username: "user", Host: "[2001:0db8:85a3:0000:0000:8a2e:0370:7334]", Port: 2222},
	}
	for idx, s := range list {
		parsed := ParseSSHUrl(s)
		t.Logf("%+v", parsed)
		if !compare(parsed, &expected[idx]) {
			t.Fatalf("%+v", &expected[idx])
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

func TestByteCountSI(t *testing.T) {
	list := []int64{
		1000,
		1001,
		1101,
		10000,
		1000000,
		1000000000,
		1000000000000,
	}
	expected := []string{
		"1.0 kB",
		"1.0 kB",
		"1.1 kB",
		"10.0 kB",
		"1.0 MB",
		"1.0 GB",
		"1.0 TB",
	}
	for idx, b := range list {
		parsed := ByteCountSI(b)
		log.Println(parsed, expected[idx])
		if parsed != expected[idx] {
			t.Fatalf("+%v", expected[idx])
		}
	}

}
