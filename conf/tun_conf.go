package conf

import "github.com/ferama/rospo/utils"

// TunnelConf is a struct that holds the tunnel configuration
type TunnnelConf struct {
	//// Tunnel conf
	Remote string
	Local  string
	// indicates if it is a forward or reverse tunnel
	Forward bool
}

// Builds a remote endpoint object from the Remote string
func (c *TunnnelConf) GetRemotEndpoint() *utils.Endpoint {
	return utils.NewEndpoint(c.Remote)
}

// Builds a locale endpoint object from the Local string
func (c *TunnnelConf) GetLocalEndpoint() *utils.Endpoint {
	return utils.NewEndpoint(c.Local)
}
