package utils

import (
	"log"
	"os/user"
	"strconv"
	"strings"
)

type sshUrl struct {
	Username string
	Host     string
	Port     int
}

func ParseSSHUrl(url string) *sshUrl {
	parts := strings.Split(url, "@")

	usr, _ := user.Current()
	conf := &sshUrl{}

	var host string

	if len(parts) == 2 {
		conf.Username = parts[0]
		host = parts[1]
	} else {
		conf.Username = usr.Username
		host = parts[0]
	}

	hostParts := strings.Split(host, ":")
	if len(hostParts) == 2 {
		port, err := strconv.Atoi(hostParts[1])
		if err != nil {
			log.Fatalln(err)
		}
		conf.Host = hostParts[0]
		conf.Port = port
	} else {
		conf.Host = host
		conf.Port = 22
	}

	return conf
}
