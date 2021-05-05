package conf

import "github.com/ferama/rospo/pkg/utils"

// TunnelConf is a struct that holds the tunnel configuration
type TunnelConf struct {
	//// Tunnel conf
	Remote string `yaml:"remote"`
	Local  string `yaml:"local"`
	// indicates if it is a forward or reverse tunnel
	Forward bool `yaml:"forward"`
}

// GetRemotEndpoint Builds a remote endpoint object from the Remote string
func (c *TunnelConf) GetRemotEndpoint() *utils.Endpoint {
	return utils.NewEndpoint(c.Remote)
}

// GetLocalEndpoint Builds a locale endpoint object from the Local string
func (c *TunnelConf) GetLocalEndpoint() *utils.Endpoint {
	return utils.NewEndpoint(c.Local)
}
