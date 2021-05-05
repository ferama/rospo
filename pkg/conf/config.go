package conf

import (
	"errors"
	"os"

	"gopkg.in/yaml.v2"
)

// Config holds all the config values
type Config struct {
	SshClient *SshClientConf `yaml:"sshclient"`
	Tunnel    []*TunnelConf  `yaml:"tunnel"`
	SshD      *SshDConf      `yaml:"sshd"`
	Pipe      []*PipeConf    `yaml:"pipe"`
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
		&SshClientConf{
			Insecure: false,
		},
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
