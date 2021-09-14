package pipe

import "strings"

type parsedRemote struct {
	Scheme string
	Data   string
}

func parseRemote(remote string) *parsedRemote {
	p := &parsedRemote{}

	parts := strings.Split(remote, "://")
	if len(parts) == 1 {
		p.Scheme = "tcp"
		p.Data = remote
		return p
	}
	p.Scheme = parts[0]
	p.Data = strings.Join(parts[1:], ":")

	return p
}
