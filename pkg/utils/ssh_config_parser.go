package utils

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/kevinburke/ssh_config"
)

type NodeConfig struct {
	Host                  string
	Port                  int
	HostName              string
	User                  string
	IdentityFile          string
	StrictHostKeyChecking bool
	UserKnownHostsFile    string
	ProxyJump             string
}

type SSHConfigParser struct{}

func NewSSHConfigParser() *SSHConfigParser {
	return &SSHConfigParser{}
}

func (s *SSHConfigParser) parseContent(f *os.File) ([]NodeConfig, error) {
	nodes := []NodeConfig{}

	cfg, err := ssh_config.Decode(f)
	if err != nil {
		return nodes, err
	}
	for _, host := range cfg.Hosts {
		for _, pattern := range host.Patterns {
			if pattern.String() == "*" { // Skip wildcard entries
				continue
			}

			nodeConf := NodeConfig{
				Host:                  pattern.String(),
				Port:                  22,                // Default Port
				IdentityFile:          "~/.ssh/identity", // Default IdentityFile
				UserKnownHostsFile:    "~/.ssh/known_hosts",
				StrictHostKeyChecking: true, // Default StrictHostKeyChecking
				ProxyJump:             "",
			}

			// Iterate over the configuration lines inside the host block
			for _, node := range host.Nodes {
				line := strings.TrimSpace(node.String())

				// Ignore comments
				if strings.HasPrefix(line, "#") || line == "" {
					continue
				}

				fields := strings.Fields(line)
				if len(fields) < 2 {
					continue
				}
				key, value := fields[0], strings.Join(fields[1:], " ") // Join in case of multiple values

				switch strings.ToLower(key) {
				case "hostname":
					nodeConf.HostName = value
				case "port":
					if port, err := strconv.Atoi(value); err == nil {
						nodeConf.Port = port
					} else {
						return nodes, fmt.Errorf("invalid value for Port: %s", value)
					}
				case "user":
					nodeConf.User = value
				case "identityfile":
					nodeConf.IdentityFile = value
				case "userknownhostsfile":
					nodeConf.UserKnownHostsFile = value
				case "stricthostkeychecking":
					if strings.ToLower(value) == "no" || strings.ToLower(value) == "false" {
						nodeConf.StrictHostKeyChecking = false
					} else if strings.ToLower(value) == "yes" || strings.ToLower(value) == "true" {
						nodeConf.StrictHostKeyChecking = true
					} else { // Invalid value
						return nodes, fmt.Errorf("invalid value for StrictHostKeyChecking: %s", value)
					}
				case "proxyjump":
					nodeConf.ProxyJump = value

				}
			}
			nodes = append(nodes, nodeConf)
		}
	}

	return nodes, nil
}

func (s *SSHConfigParser) Parse() ([]NodeConfig, error) {
	f, _ := os.Open(filepath.Join(os.Getenv("HOME"), ".ssh", "config"))
	return s.parseContent(f)
}
