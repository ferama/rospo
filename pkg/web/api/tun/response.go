package tunapi

import (
	"net"

	"github.com/ferama/rospo/pkg/utils"
)

type tunResponseItem struct {
	ID               int            `json:"Id"`
	Listener         net.Addr       `json:"Listener"`
	IsListenerLocal  bool           `json:"IsListenerLocal"`
	Endpoint         utils.Endpoint `json:"Endpoint"`
	ClientsCount     int            `json:"ClientsCount"`
	IsStoppable      bool           `json:"IsStoppable"`
	Throughput       int64          `json:"Throughput"`
	ThroughputString string         `json:"ThroughputString"`
}
