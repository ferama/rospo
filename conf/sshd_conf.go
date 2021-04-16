package conf

// SshDConf holds the sshd configuration
type SshDConf struct {
	Identity          string
	AuthorizedKeyFile string
	// The tcp port the sshd server will listen too
	Port string
}
