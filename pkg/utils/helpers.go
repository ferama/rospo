package utils

import (
	"bufio"
	"log"
	"os"
	"os/user"
	"path/filepath"
	"runtime"
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

// GetUserDefaultShell try to get the best shell for the user
func GetUserDefaultShell(username string) string {
	if runtime.GOOS == "windows" {
		return "c:\\windows\\system32\\windowspowershell\\v1.0\\powershell.exe"
	}
	fallback := "/bin/sh"

	file, err := os.Open("/etc/passwd")
	if err != nil {
		return fallback
	}
	defer file.Close()

	lines := bufio.NewReader(file)
	for {
		line, _, err := lines.ReadLine()
		if err != nil {
			break
		}
		fs := strings.Split(string(line), ":")
		if len(fs) != 7 {
			continue
		}
		if fs[0] != username {
			continue
		}
		shell := fs[6]
		return shell
	}

	return fallback
}
