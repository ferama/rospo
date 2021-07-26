package pipe

// PipeConf holds the forward configuration
type PipeConf struct {
	Local  string `yaml:"local"`
	Remote string `yaml:"remote"`
}
