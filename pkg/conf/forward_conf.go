package conf

// ForwardConf holds the forward configuration
type ForwardConf struct {
	Remote string `yaml:"remote"`
	Local  string `yaml:"local"`
}
