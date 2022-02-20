package utils

import (
	"bufio"
	"errors"
	"io"
	"log"
	"os"
	"os/user"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"sync"
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

// copyBuffer is the actual implementation of Copy and CopyBuffer.
func copyBuffer(dst io.Writer, src io.Reader, wch chan int64) (err error) {
	var buf []byte
	size := 32 * 1024
	if l, ok := src.(*io.LimitedReader); ok && int64(size) > l.N {
		if l.N < 1 {
			size = 1
		} else {
			size = int(l.N)
		}
	}
	buf = make([]byte, size)
	for {
		nr, er := src.Read(buf)
		if nr > 0 {
			nw, ew := dst.Write(buf[0:nr])
			if nw < 0 || nr < nw {
				nw = 0
				if ew == nil {
					ew = errors.New("invalid write result")
				}
			}
			// written += int64(nw)
			select {
			case wch <- int64(nw):
			default:
			}
			if ew != nil {
				err = ew
				break
			}
			if nr != nw {
				err = io.ErrShortWrite
				break
			}
		}
		if er != nil {
			if er != io.EOF {
				err = er
			}
			break
		}
	}
	return err
}

// CopyConnWithOnClose copy packets from c1 to c2 and viceversa. Calls the onClose function
// when the connection is interrupted
func CopyConnWithOnClose(
	c1 io.ReadWriteCloser,
	c2 io.ReadWriteCloser,
	byteswritten chan int64,
	onClose func()) {

	w1 := make(chan int64)
	w2 := make(chan int64)

	var once sync.Once

	connClose := func() {
		c1.Close()
		c2.Close()
		onClose()
	}

	go func() {
		for nw := range w1 {
			select {
			case byteswritten <- int64(nw):
			default:
			}
		}
	}()

	go func() {
		for nw := range w2 {
			select {
			case byteswritten <- int64(nw):
			default:
			}
		}
	}()

	go func() {
		copyBuffer(c1, c2, w1)
		close(w1)
		once.Do(connClose)
	}()

	go func() {
		copyBuffer(c2, c1, w2)
		close(w2)
		once.Do(connClose)
	}()

}

// CopyConn copy packets from c1 to c2 and viceversa
func CopyConn(c1 io.ReadWriteCloser, c2 io.ReadWriteCloser, byteswritten chan int64) {
	CopyConnWithOnClose(c1, c2, byteswritten, func() {})
}
