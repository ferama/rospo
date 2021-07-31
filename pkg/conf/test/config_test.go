package conf

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/ferama/rospo/pkg/conf"
)

func TestSshDShellDisabledDefault(t *testing.T) {
	wd, _ := os.Getwd()
	path := filepath.Join(wd, "sshd.yaml")

	cfg, err := conf.LoadConfig(path)
	if err != nil {
		t.Fatalf("can't parse config")
	}
	// t.Logf("VALUE: %t", cfg.SshD.DisableShell)
	if cfg.SshD.DisableShell {
		t.Fatalf("disable shell")
	}
}
