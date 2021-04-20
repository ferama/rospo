package conf

import "github.com/ferama/rospo/utils"

type JumpHostConf struct {
	// user@server:port
	URI      string `yaml:"uri"`
	Identity string `yaml:"identity"`
}

// SshClientConf holds the ssh client configuration
type SshClientConf struct {
	Username string `yaml:"username"`
	Identity string `yaml:"identity"`
	Server   string `yaml:"server"`
	// it this value is true host keys are not checked
	// against known_hosts file
	Insecure  bool            `yaml:"insecure"`
	JumpHosts []*JumpHostConf `yaml:"jump_hosts"`
}

// Builds a server endpoint object from the Server string
func (c *SshClientConf) GetServerEndpoint() *utils.Endpoint {
	return utils.NewEndpoint(c.Server)
}
