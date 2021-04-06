package sshd

import (
	"encoding/binary"
	"fmt"
	"io"
	"log"
	"os"
	"os/exec"
	"os/user"
	"sync"
	"syscall"

	"github.com/creack/pty"
	"golang.org/x/crypto/ssh"
)

type envRequestPayload struct {
	Name  string
	Value string
}

// Start assigns a pseudo-terminal tty os.File to c.Stdin, c.Stdout,
// and c.Stderr, calls c.Start, and returns the File of the tty's
// corresponding pty.
func ptyRun(c *exec.Cmd, tty *os.File) (err error) {
	defer tty.Close()
	c.Stdout = tty
	c.Stdin = tty
	c.Stderr = tty
	c.SysProcAttr = &syscall.SysProcAttr{
		Setctty: true,
		Setsid:  true,
	}
	return c.Start()
}

func handleChannelSession(c ssh.NewChannel) {
	channel, requests, err := c.Accept()
	if err != nil {
		log.Printf("[SSHD] could not accept channel (%s)", err)
		return
	}
	// allocate a terminal for this channel
	log.Print("[SSHD] creating pty...")
	// Create new pty
	f, tty, err := pty.Open()
	if err != nil {
		log.Printf("[SSHD] could not start pty (%s)", err)
		return
	}
	var shell string
	shell = os.Getenv("SHELL")
	if shell == "" {
		shell = DEFAULT_SHELL
	}

	env := map[string]string{}

	for req := range requests {
		// log.Printf("%v %s", req.Payload, req.Payload)
		ok := false
		switch req.Type {
		// run a command on remote host and exit
		case "exec":
			ok = true
			command := string(req.Payload[4 : req.Payload[3]+4])
			cmd := exec.Command(shell, []string{"-c", command}...)

			cmd.Stdout = channel
			cmd.Stderr = channel
			cmd.Stdin = channel

			err := cmd.Start()
			if err != nil {
				log.Printf("[SSHD] could not start command (%s)", err)
				continue
			}

			// teardown session
			go func() {
				_, err := cmd.Process.Wait()
				if err != nil {
					log.Printf("[SSHD] failed to exit bash (%s)", err)
				}
				channel.Close()
				log.Printf("[SSHD] session closed")
			}()
		// request an interactive shell
		case "shell":
			cmd := exec.Command(shell)
			// cmd.Env = []string{"TERM=xterm"}
			envVal := make([]string, 0, len(env))
			for k, v := range env {
				envVal = append(envVal, fmt.Sprintf("%s=%s", k, v))
			}
			envVal = append(envVal, "TERM=xterm")

			usr, _ := user.Current()
			envVal = append(envVal, fmt.Sprintf("HOME=%s", usr.HomeDir))
			cmd.Env = envVal
			log.Printf("[SSHD] env %s", envVal)

			err := ptyRun(cmd, tty)
			if err != nil {
				log.Printf("[SSHD] %s", err)
			}

			// Teardown session
			var once sync.Once
			close := func() {
				channel.Close()
				log.Printf("[SSHD] session closed")
			}

			// Pipe session to bash and visa-versa
			go func() {
				_, err := io.Copy(channel, f)
				if err != nil {
					log.Println(fmt.Sprintf("[SSHD] error while copy: %s", err))
				}
				cmd.Process.Wait()
				once.Do(close)
			}()

			go func() {
				_, err := io.Copy(f, channel)
				if err != nil {
					log.Println(fmt.Sprintf("[SSHD] error while copy: %s", err))
				}
				cmd.Process.Wait()
				once.Do(close)
			}()

			// We don't accept any commands (Payload),
			// only the default shell.
			if len(req.Payload) == 0 {
				ok = true
			}

		case "pty-req":
			// Responding 'ok' here will let the client
			// know we have a pty ready for input
			ok = true
			// Parse body...
			termLen := req.Payload[3]
			termEnv := string(req.Payload[4 : termLen+4])
			w, h := parseDims(req.Payload[termLen+4:])
			SetWinsize(f.Fd(), w, h)
			log.Printf("[SSHD] pty-req '%s'", termEnv)

		case "window-change":
			w, h := parseDims(req.Payload)
			SetWinsize(f.Fd(), w, h)
			continue //no response

		case "env":
			ureq := envRequestPayload{}
			if err := ssh.Unmarshal(req.Payload, &ureq); err != nil {
				log.Printf("[SSHD] invalid env payload: %s", req.Payload)
			}
			env[ureq.Name] = ureq.Value
			continue
		}

		if !ok {
			// log.Printf("declining %s request... %s", req.Type, req.Payload)
			log.Printf("[SSHD] declining %s request... ", req.Type)
		}

		req.Reply(ok, nil)
	}
}

// parseDims extracts two uint32s from the provided buffer.
func parseDims(b []byte) (uint32, uint32) {
	w := binary.BigEndian.Uint32(b)
	h := binary.BigEndian.Uint32(b[4:])
	return w, h
}
