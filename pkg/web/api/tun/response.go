package tunapi

import "net"

type responseItem struct {
	ID   int      `json:"Id"`
	Addr net.Addr `json:"Addr"`
}
