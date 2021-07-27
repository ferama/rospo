package tunapi

import (
	"net"

	"github.com/ferama/rospo/pkg/utils"
)

type responseItem struct {
	ID              int            `json:"Id"`
	Listener        net.Addr       `json:"Listener"`
	IsListenerLocal bool           `json:"IsListenerLocal"`
	Endpoint        utils.Endpoint `json:"Endpoint"`
	ClientsCount    int            `json:"ClientsCount"`
}
