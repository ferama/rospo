package pipeapi

import (
	"net"
)

type pipeResponseItem struct {
	ID           int      `json:"Id"`
	Listener     net.Addr `json:"Listener"`
	Endpoint     string   `json:"Endpoint"`
	ClientsCount int      `json:"ClientsCount"`
	IsStoppable  bool     `json:"IsStoppable"`
}
