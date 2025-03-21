package utils

import (
	"os"
	"testing"
)

func TestSSHConfigParser(t *testing.T) {
	parser := NewSSHConfigParser()
	f, _ := os.Open("testdata/ssh_config")
	defer f.Close()

	nodes, err := parser.parseContent(f)
	if err != nil {
		t.Errorf("Error parsing SSH config: %v", err)
	}

	expected := []NodeConfig{
		{
			Host:                  "test1",
			Port:                  22,
			HostName:              "127.0.0.1",
			User:                  "user1",
			IdentityFile:          "~/.ssh/identity",
			StrictHostKeyChecking: true,
			UserKnownHostsFile:    "~/.ssh/known_hosts",
			ProxyJump:             "",
		},
		{
			Host:                  "test2",
			Port:                  2222,
			HostName:              "myhost.link",
			User:                  "user2",
			IdentityFile:          "~/identities/myhost",
			StrictHostKeyChecking: true,
			UserKnownHostsFile:    "~/.ssh/known_hosts",
			ProxyJump:             "",
		},
		{
			Host:                  "test3",
			Port:                  2222,
			HostName:              "myhost.link",
			User:                  "user2",
			IdentityFile:          "~/identities/myhost",
			StrictHostKeyChecking: false,
			UserKnownHostsFile:    "/dev/null",
			ProxyJump:             "test2",
		},
	}

	for i, node := range nodes {
		if node.Host != expected[i].Host {
			t.Errorf("Host: Expected %s, got %s", expected[i].Host, node.Host)
		}
		if node.Port != expected[i].Port {
			t.Errorf("Port: Expected %d, got %d", expected[i].Port, node.Port)
		}
		if node.HostName != expected[i].HostName {
			t.Errorf("HostName: Expected %s, got %s", expected[i].HostName, node.HostName)
		}
		if node.User != expected[i].User {
			t.Errorf("User: Expected %s, got %s", expected[i].User, node.User)
		}
		if node.IdentityFile != expected[i].IdentityFile {
			t.Errorf("IdentityFile: Expected %s, got %s", expected[i].IdentityFile, node.IdentityFile)
		}
		if node.StrictHostKeyChecking != expected[i].StrictHostKeyChecking {
			t.Errorf("StrictHostKeyChecking: Expected %t, got %t", expected[i].StrictHostKeyChecking, node.StrictHostKeyChecking)
		}
		if node.UserKnownHostsFile != expected[i].UserKnownHostsFile {
			t.Errorf("UserKnownHostsFile: Expected %s, got %s", expected[i].UserKnownHostsFile, node.UserKnownHostsFile)
		}
		if node.ProxyJump != expected[i].ProxyJump {
			t.Errorf("ProxyJump: Expected %s, got %s", expected[i].ProxyJump, node.ProxyJump)
		}
	}
	// t.Logf("%+v", nodes)
}
