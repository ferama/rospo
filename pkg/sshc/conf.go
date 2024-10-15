package sshc

import "github.com/ferama/rospo/pkg/utils"

// JumpHostConf holds a jump host configuration
type JumpHostConf struct {
	// user@server:port
	URI      string `yaml:"uri"`
	Identity string `yaml:"identity"`
	Password string `yaml:"password"`
}

// SshClientConf holds the ssh client configuration
type SshClientConf struct {
	Identity   string `yaml:"identity"`
	Password   string `yaml:"password"`
	KnownHosts string `yaml:"known_hosts"`
	ServerURI  string `yaml:"server"`
	// it this value is true host keys are not checked
	// against known_hosts file
	Insecure  bool            `yaml:"insecure"`
	Quiet     bool            `yaml:"quiet"`
	JumpHosts []*JumpHostConf `yaml:"jump_hosts"`
}

type SocksProxyConf struct {
	ListenAddress string `yaml:"listen_address"`
	// use a dedicated ssh client. if nil use the global one
	SshClientConf *SshClientConf `yaml:"sshclient"`
}

type DnsProxyConf struct {
	ListenAddress    string  `yaml:"listen_address"`
	RemoteDnsAddress *string `yaml:"remote_dns_address"`
	// use a dedicated ssh client. if nil use the global one
	SshClientConf *SshClientConf `yaml:"sshclient"`
}

// GetServerEndpoint Builds a server endpoint object from the Server string
func (c *SshClientConf) GetServerEndpoint() *utils.Endpoint {
	return utils.NewEndpoint(c.ServerURI)
}
