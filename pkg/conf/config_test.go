package conf

import (
	"path/filepath"
	"testing"
)

func TestSshDShellDisabledDefault(t *testing.T) {
	path := filepath.Join("testdata", "sshd.yaml")

	cfg, err := LoadConfig(path)
	if err != nil {
		t.Fatalf("can't parse config")
	}
	if cfg.SshD.DisableShell {
		t.Fatalf("disable shell")
	}
}

func TestEmptySshc(t *testing.T) {
	path := filepath.Join("testdata", "sshd.yaml")

	cfg, err := LoadConfig(path)
	if err != nil {
		t.Fatalf("can't parse config")
	}
	if cfg.SshClient != nil {
		t.Fatalf("sshclient should be nil")
	}
	// if cfg.SshD.DisableShell {
	// 	t.Fatalf("disable shell")
	// }
}
