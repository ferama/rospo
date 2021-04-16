package conf

// Config holds all the config values
type Config struct {
	SshClient *SshClientConf
	Tunnel    *TunnnelConf
	SshD      *SshDConf
}
