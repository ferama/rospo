package utils

import (
	"bufio"
	"fmt"
	"log"
	"net"
	"os"
	"os/user"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
)

const (
	defaultPort = 22
	defaultHost = "127.0.0.1"
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

	var hostPort string

	if len(parts) == 2 {
		conf.Username = parts[0]
		hostPort = parts[1]
	} else {
		conf.Username = usr.Username
		hostPort = parts[0]
	}

	host, port, err := net.SplitHostPort(hostPort)
	if err != nil {
		// error could be "missing port in address" so try again appending defaultPort
		host, port, err = net.SplitHostPort(fmt.Sprintf("%s:%d", hostPort, defaultPort))
		if err != nil {
			log.Fatalln(err)
		}
	}

	conf.Host = defaultHost
	if host != "" {
		conf.Host = host

		ip := net.ParseIP(host)
		if ip != nil { // it could be a domain name
			if ip.To4() == nil {
				conf.Host = fmt.Sprintf("[%s]", host)
			}
		}
	}

	conf.Port = defaultPort
	if port != "" {
		port, err := strconv.Atoi(port)
		if err != nil {
			log.Fatalln(err)
		}
		conf.Port = port
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

func ByteCountSI(b int64) string {
	const unit = 1000
	if b < unit {
		return fmt.Sprintf("%d B", b)
	}
	div, exp := int64(unit), 0
	for n := b / unit; n >= unit; n /= unit {
		div *= unit
		exp++
	}
	return fmt.Sprintf("%.1f %cB",
		float64(b)/float64(div), "kMGTPE"[exp])
}
