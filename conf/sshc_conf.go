package conf

import "github.com/ferama/rospo/utils"

// SshClientConf holds the ssh client configuration
type SshClientConf struct {
	Username string
	Identity string
	Server   string
	// it this value is true host keys are not checked
	// against known_hosts file
	Insecure bool
	JumpHost string
}

// Builds a server endpoint object from the Server string
func (c *SshClientConf) GetServerEndpoint() *utils.Endpoint {
	return utils.NewEndpoint(c.Server)
}
