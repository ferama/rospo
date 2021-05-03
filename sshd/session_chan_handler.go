package sshd

import (
	"encoding/binary"
	"fmt"
	"log"
	"os/exec"
	"os/user"
	"sync"

	"github.com/ferama/rospo/rpty"
	"github.com/ferama/rospo/utils"
	"golang.org/x/crypto/ssh"
)

func handleChannelSession(c ssh.NewChannel) {
	channel, requests, err := c.Accept()
	if err != nil {
		log.Printf("[SSHD] could not accept channel (%s)", err)
		return
	}

	var shell string

	usr, err := user.Current()
	if err != nil {
		panic(err)
	}
	shell = utils.GetUserDefaultShell(usr.Username)

	// allocate a terminal for this channel
	log.Print("[SSHD] creating pty...")
	// Create new pty
	pty, err := rpty.New()

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
				if err := pty.Run(cmd); err != nil {
					log.Printf("[SSHD] %s", err)
				}
				sessionClientServe(channel, pty)

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
						cmd.Process.Kill()
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
			pty.Resize(uint16(w), uint16(h))
			log.Printf("[SSHD] pty-req '%s'", termEnv)

		case "window-change":
			w, h := parseDims(req.Payload)
			pty.Resize(uint16(w), uint16(h))
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

func sessionClientServe(channel ssh.Channel, pty rpty.Pty) {
	// Teardown session
	var once sync.Once
	close := func() {
		channel.Close()
		pty.Close()
		log.Printf("[SSHD] client session closed")
	}

	// Pipe session to shell and vice-versa
	go func() {
		_, err := pty.WriteTo(channel)
		if err != nil {
			log.Println(fmt.Sprintf("[SSHD] error while copy from channel: %s", err))
		}
		once.Do(close)
	}()

	go func() {
		_, err := pty.ReadFrom(channel)
		if err != nil {
			log.Println(fmt.Sprintf("[SSHD] error while copy to channel: %s", err))
		}
		once.Do(close)
	}()
}
