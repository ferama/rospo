package conf

import (
	"errors"
	"os"

	"github.com/ferama/rospo/pkg/pipe"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/ferama/rospo/pkg/sshd"
	"github.com/ferama/rospo/pkg/tun"
	"github.com/ferama/rospo/pkg/web"
	"gopkg.in/yaml.v2"
)

// Config holds all the config values
type Config struct {
	SshClient *sshc.SshClientConf `yaml:"sshclient"`
	Tunnel    []*tun.TunnelConf   `yaml:"tunnel"`
	SshD      *sshd.SshDConf      `yaml:"sshd"`
	Pipe      []*pipe.PipeConf    `yaml:"pipe"`
	Web       *web.WebConf        `yaml:"web"`
}

// LoadConfig parses the [config].yaml file and loads its values
// into the Config struct
func LoadConfig(filePath string) (*Config, error) {
	f, err := os.Open(filePath)
	if err != nil {
		return nil, err
	}
	defer f.Close()

	// set some reasonable defaults
	cfg := Config{
		&sshc.SshClientConf{
			Insecure: false,
		},
		nil,
		nil,
		nil,
		nil,
	}

	decoder := yaml.NewDecoder(f)
	err = decoder.Decode(&cfg)
	if err != nil {
		return nil, err
	}

	if cfg.SshD == nil && cfg.Tunnel == nil {
		return nil, errors.New("invalid config file: you need to fill at least one of the `sshd` or `tunnel` sections")
	}

	return &cfg, nil
}
