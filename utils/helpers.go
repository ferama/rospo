package utils

import (
	"log"
	"os/user"
	"path/filepath"
	"strconv"
	"strings"
)

type sshUrl struct {
	Username string
	Host     string
	Port     int
}

// ParseSSHUrl build an sshUrl object from an url string
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
		if hostParts[0] == "" {
			conf.Host = "127.0.0.1"
		} else {
			conf.Host = hostParts[0]

		}
		conf.Port = port
	} else {
		conf.Host = host
		conf.Port = 22
	}

	return conf
}

// ExpandUserHome resolve paths like "~/.ssh/id_rsa"
func ExpandUserHome(path string) (string, error) {
	usr, err := user.Current()
	if err != nil {
		return "", err
	}
	ret := path

	// supports paths like "~/.ssh/id_rsa"
	if strings.HasPrefix(path, "~/") {
		ret = filepath.Join(usr.HomeDir, path[2:])
	}
	return ret, nil
}
