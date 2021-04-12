package tun

import (
	"fmt"

	"github.com/ferama/rospo/utils"
)

type Endpoint struct {
	Host string
	Port int
}

func NewEndpoint(s string) *Endpoint {
	parsed := utils.ParseSSHUrl(s)
	e := &Endpoint{
		Host: parsed.Host,
		Port: parsed.Port,
	}
	return e
}

func (endpoint *Endpoint) String() string {
	return fmt.Sprintf("%s:%d", endpoint.Host, endpoint.Port)
}
