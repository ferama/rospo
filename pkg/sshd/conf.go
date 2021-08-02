package sshd

// SshDConf holds the sshd configuration
type SshDConf struct {
	Key                string `yaml:"server_key"`
	AuthorizedKeysFile string `yaml:"authorized_keys"`
	// The address the sshd server will listen too
	ListenAddress string `yaml:"listen_address"`
	// if true the exec,shell requests will be ignored
	DisableShell bool `yaml:"disable_shell"`
}
