package utils

import (
	"fmt"
)

// Endpoint holds the tunnel endpoint details
type Endpoint struct {
	Host string
	Port int
}

// NewEndpoint builds an Endpoint object
func NewEndpoint(s string) *Endpoint {
	parsed := ParseSSHUrl(s)
	e := &Endpoint{
		Host: parsed.Host,
		Port: parsed.Port,
	}
	return e
}

// String returns the string representation of the endpoint
func (endpoint *Endpoint) String() string {
	return fmt.Sprintf("%s:%d", endpoint.Host, endpoint.Port)
}
