package sshd

// SshDConf holds the sshd configuration
type SshDConf struct {
	Key               string   `yaml:"server_key"`
	AuthorizedKeysURI []string `yaml:"authorized_keys"`

	AuthorizedPassword string `yaml:"authorized_password"`
	// The address the sshd server will listen too
	ListenAddress string `yaml:"listen_address"`
	// if true the exec,shell requests will be ignored
	DisableShell bool `yaml:"disable_shell"`
	// if true no banner will be displayed while interacting
	// with the sshd server
	DisableBanner bool `yaml:"disable_banner"`
	// if true all auth mechanism will be disabled
	// use with caution
	DisableAuth bool `yaml:"disable_auth"`
	// shell executable. Leave empty for default behaviour
	ShellExecutable string `yaml:"shell_executable"`
}
