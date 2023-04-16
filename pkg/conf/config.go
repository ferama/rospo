package conf

import (
	"os"

	"github.com/ferama/rospo/pkg/sshc"
	"github.com/ferama/rospo/pkg/sshd"
	"github.com/ferama/rospo/pkg/tun"
	"github.com/ferama/rospo/pkg/web"
	"gopkg.in/yaml.v3"
)

// Config holds all the config values
type Config struct {
	SshClient  *sshc.SshClientConf  `yaml:"sshclient"`
	Tunnel     []*tun.TunnelConf    `yaml:"tunnel"`
	SshD       *sshd.SshDConf       `yaml:"sshd"`
	Web        *web.WebConf         `yaml:"web"`
	SocksProxy *sshc.SocksProxyConf `yaml:"socksproxy"`
}

// LoadConfig parses the [config].yaml file and loads its values
// into the Config struct
func LoadConfig(filePath string) (*Config, error) {
	f, err := os.Open(filePath)
	if err != nil {
		return nil, err
	}
	defer f.Close()

	cfg := Config{
		nil,
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

	return &cfg, nil
}
