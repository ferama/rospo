package tunapi

import "net"

type item struct {
	ID   int      `json:"Id"`
	Addr net.Addr `json:"Addr"`
}
