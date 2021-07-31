package utils

import (
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
		"192.168.0.1",
		"192.168.0.1:2222",
		":22",
		"user-name@192.168.0.1:2222",
	}

	expected := []sshUrl{
		{Username: "user", Host: "192.168.0.1", Port: 22},
		{Username: currentUser.Username, Host: "192.168.0.1", Port: 22},
		{Username: currentUser.Username, Host: "192.168.0.1", Port: 2222},
		{Username: currentUser.Username, Host: "127.0.0.1", Port: 22},
		{Username: "user-name", Host: "192.168.0.1", Port: 2222},
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
}
