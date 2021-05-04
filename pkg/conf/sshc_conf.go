package conf

import "github.com/ferama/rospo/pkg/utils"

// JumpHostConf holds a jump host configuration
type JumpHostConf struct {
	// user@server:port
	URI      string `yaml:"uri"`
	Identity string `yaml:"identity"`
}

// SshClientConf holds the ssh client configuration
type SshClientConf struct {
	Identity   string `yaml:"identity"`
	KnownHosts string `yaml:"known_hosts"`
	ServerURI  string `yaml:"server"`
	// it this value is true host keys are not checked
	// against known_hosts file
	Insecure  bool            `yaml:"insecure"`
	JumpHosts []*JumpHostConf `yaml:"jump_hosts"`
}

// Builds a server endpoint object from the Server string
func (c *SshClientConf) GetServerEndpoint() *utils.Endpoint {
	return utils.NewEndpoint(c.ServerURI)
}
