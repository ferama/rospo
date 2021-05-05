package conf

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/ferama/rospo/pkg/conf"
)

func TestMinimumRequired(t *testing.T) {
	wd, _ := os.Getwd()
	path := filepath.Join(wd, "missing_tunnel_and_sshd.yaml")

	if _, err := conf.LoadConfig(path); err == nil {
		t.Fatalf("missing_tunnel_and_sshd")
	}

	if _, err := conf.LoadConfig("not_exists_path"); err == nil {
		t.Fatalf("missing file")
	}
}
