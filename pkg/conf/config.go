package conf

import (
	"log"
	"os"
	"os/user"
	"path/filepath"

	"github.com/ferama/rospo/pkg/utils"
	"gopkg.in/yaml.v2"
)

// Config holds all the config values
type Config struct {
	SshClient *SshClientConf `yaml:"sshclient"`
	Tunnel    []*TunnnelConf `yaml:"tunnel"`
	SshD      *SshDConf      `yaml:"sshd"`
	Forward   []*ForwardConf `yaml:"forward"`
}

// LoadConfig parses the [config].yaml file and loads its values
// into the Config struct
func LoadConfig(filePath string) *Config {
	f, err := os.Open(filePath)
	if err != nil {
		log.Fatalf("Error while reading config file: %s", err)
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

	var knownHostsPath string
	if cfg.SshClient.KnownHosts == "" {
		usr, _ := user.Current()
		knownHostsPath = filepath.Join(usr.HomeDir, ".ssh", "known_hosts")
	} else {
		knownHostsPath, _ = utils.ExpandUserHome(cfg.SshClient.KnownHosts)
	}
	cfg.SshClient.KnownHosts = knownHostsPath

	if err != nil {
		log.Fatalf("Error while parsing config file: %s", err)
	}
	return &cfg
}
