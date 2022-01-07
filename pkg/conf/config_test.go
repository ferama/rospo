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
}

func TestSshcSecure(t *testing.T) {
	path := filepath.Join("testdata", "sshc.yaml")

	cfg, err := LoadConfig(path)
	if err != nil {
		t.Fatalf("can't parse config")
	}
	if cfg.SshClient.Insecure != false {
		t.Fatalf("sshclient should be secure")
	}
}

func TestSshcInsecure(t *testing.T) {
	path := filepath.Join("testdata", "sshc_insecure.yaml")

	cfg, err := LoadConfig(path)
	if err != nil {
		t.Fatalf("can't parse config")
	}
	if cfg.SshClient.Insecure != true {
		t.Fatalf("sshclient should be insecure")
	}
}

func TestSshcSecureDefault(t *testing.T) {
	path := filepath.Join("testdata", "sshc_secure_default.yaml")

	cfg, err := LoadConfig(path)
	if err != nil {
		t.Fatalf("can't parse config")
	}
	if cfg.SshClient.Insecure != false {
		t.Fatalf("sshclient should be secure by default")
	}
}
