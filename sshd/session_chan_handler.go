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

func ptyServe(channel ssh.Channel, pty *os.File, cmd *exec.Cmd) {
	// Teardown session
	var once sync.Once
	close := func() {
		channel.Close()
		cmd.Process.Wait()
		log.Printf("[SSHD] session closed")
	}

	// Pipe session to bash and visa-versa
	go func() {
		_, err := io.Copy(channel, pty)
		if err != nil {
			log.Println(fmt.Sprintf("[SSHD] error while copy from channel: %s", err))
		}
		once.Do(close)
	}()

	go func() {
		_, err := io.Copy(pty, channel)
		if err != nil {
			log.Println(fmt.Sprintf("[SSHD] error while copy to channel: %s", err))
		}
		once.Do(close)
	}()
}

func handleChannelSession(c ssh.NewChannel) {
	channel, requests, err := c.Accept()
	if err != nil {
		log.Printf("[SSHD] could not accept channel (%s)", err)
		return
	}

	var shell string
	shell = os.Getenv("SHELL")
	if shell == "" {
		shell = DEFAULT_SHELL
	}

	// allocate a terminal for this channel
	log.Print("[SSHD] creating pty...")
	// Create new pty
	f, tty, err := pty.Open()
	if err != nil {
		log.Printf("[SSHD] could not start pty (%s)", err)
		return
	}

	env := map[string]string{}
	ptyRequested := false

	for req := range requests {
		// log.Printf("### %v %s", req.Type, req.Payload)
		ok := false
		switch req.Type {
		case "shell", "exec":
			var cmd *exec.Cmd

			if req.Type == "shell" {
				cmd = exec.Command(shell)
			} else {
				var payload = struct{ Value string }{}
				ssh.Unmarshal(req.Payload, &payload)
				command := payload.Value
				cmd = exec.Command(shell, []string{"-c", command}...)
			}
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

			if ptyRequested {
				log.Println("[SSHD] running within the pty")
				if err := ptyRun(cmd, tty); err != nil {
					log.Printf("[SSHD] %s", err)
				}
				ptyServe(channel, f, cmd)

			} else {
				cmd.Stdout = channel
				cmd.Stderr = channel
				cmd.Stdin = channel
				err := cmd.Start()
				if err != nil {
					log.Printf("[SSHD] %s", err)
				}

				go func() {
					_, err := cmd.Process.Wait()
					if err != nil {
						log.Printf("[SSHD] failed to exit bash (%s)", err)
					}
					channel.Close()
					log.Printf("[SSHD] session closed")
				}()
			}

			ok = true

		case "pty-req":
			// Responding 'ok' here will let the client
			// know we have a pty ready for input
			ok = true
			ptyRequested = true
			// Parse body...
			termLen := req.Payload[3]
			termEnv := string(req.Payload[4 : termLen+4])
			w, h := parseDims(req.Payload[termLen+4:])
			setWinsize(f.Fd(), w, h)
			log.Printf("[SSHD] pty-req '%s'", termEnv)

		case "window-change":
			w, h := parseDims(req.Payload)
			setWinsize(f.Fd(), w, h)
			continue //no response

		case "env":
			var payload = struct{ Name, Value string }{}

			if err := ssh.Unmarshal(req.Payload, &payload); err != nil {
				log.Printf("[SSHD] invalid env payload: %s", req.Payload)
			}
			env[payload.Name] = payload.Value
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
