package utils

import "os"

// BaselineSSHConfigParser is a small test-only style adapter used by the
// migration tooling to capture parsed ssh config output from the Go codebase.
type BaselineSSHConfigParser struct {
	parser *SSHConfigParser
}

func NewBaselineSSHConfigParser() *BaselineSSHConfigParser {
	return &BaselineSSHConfigParser{parser: newSSHConfigParser()}
}

func (b *BaselineSSHConfigParser) ParseFile(path string) ([]NodeConfig, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()

	return b.parser.parseContent(f)
}
