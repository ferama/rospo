package tun

// Config holds the tunnel config values
type Config struct {
	Username string
	Identity string

	Server   string
	Remote   string
	Local    string
	JumpHost string

	// indicates if it is a forward or reverse tunnel
	Forward bool

	// it this value is true host keys are not checked
	// against known_hosts file
	Insecure bool
}

// Builds a server endpoint object from the Server string
func (c *Config) GetServerEndpoint() *Endpoint {
	return NewEndpoint(c.Server)
}

// Builds a remote endpoint object from the Remote string
func (c *Config) GetRemotEndpoint() *Endpoint {
	return NewEndpoint(c.Remote)
}

// Builds a locale endpoint object from the Local string
func (c *Config) GetLocalEndpoint() *Endpoint {
	return NewEndpoint(c.Local)
}
