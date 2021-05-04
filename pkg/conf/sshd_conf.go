package conf

// SshDConf holds the sshd configuration
type SshDConf struct {
	Key                string `yaml:"server_key"`
	AuthorizedKeysFile string `yaml:"authorized_keys"`
	// The tcp port the sshd server will listen too
	Port string `yaml:"port"`
}
