package conn

import (
	"fmt"
	"log"
	"strconv"
	"strings"
)

type Endpoint struct {
	Host string
	Port int
}

func NewEndpoint(s string) *Endpoint {
	e := &Endpoint{}
	parts := strings.Split(s, ":")
	e.Host = parts[0]
	if len(parts) == 2 {
		port, err := strconv.Atoi(parts[1])
		if err != nil {
			log.Fatalln(err)
		}
		e.Port = port
	} else {
		e.Port = 22
	}

	// log.Println(e.String())
	return e
}

func (endpoint *Endpoint) String() string {
	return fmt.Sprintf("%s:%d", endpoint.Host, endpoint.Port)
}
