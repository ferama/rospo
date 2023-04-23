package sshd

import (
	"encoding/binary"
	"fmt"
	"io"
	"net"
	"os"
	"os/exec"
	"os/user"
	"strings"
	"sync"

	"github.com/ferama/rospo/pkg/rio"
	"github.com/ferama/rospo/pkg/rpty"
	"github.com/ferama/rospo/pkg/utils"
	"github.com/pkg/sftp"
	"golang.org/x/crypto/ssh"
)

// parseDims extracts two uint32s from the provided buffer.
func parseDims(b []byte) (uint32, uint32) {
	w := binary.BigEndian.Uint32(b)
	h := binary.BigEndian.Uint32(b[4:])
	return w, h
}

type channelHandler struct {
	sshConn *ssh.ServerConn

	chans <-chan ssh.NewChannel

	disableShell         bool
	shellExecutable      string
	disableSftpSubsystem bool
}

func newChannelHandler(
	sshConn *ssh.ServerConn,
	chans <-chan ssh.NewChannel,
	disableShell bool,
	shellExecutable string,
	disableSftpSubsystem bool,
) *channelHandler {

	return &channelHandler{
		sshConn:              sshConn,
		chans:                chans,
		disableShell:         disableShell,
		shellExecutable:      shellExecutable,
		disableSftpSubsystem: disableSftpSubsystem,
	}

}

func (s *channelHandler) handleChannelSession(c ssh.NewChannel) {
	channel, requests, err := c.Accept()
	if err != nil {
		log.Printf("could not accept channel (%s)", err)
		return
	}

	var shell string

	if s.shellExecutable == "" {
		usr, err := user.Current()
		if err != nil {
			panic(err)
		}
		shell = utils.GetUserDefaultShell(usr.Username)
	} else {
		shell = s.shellExecutable
	}

	var pty rpty.Pty
	env := map[string]string{}

	for req := range requests {
		ok := false
		switch req.Type {
		case "shell", "exec":
			if s.disableShell {
				log.Printf("declining %s request... ", req.Type)
				req.Reply(false, nil)
				continue
			}
			var cmd *exec.Cmd

			if req.Type == "shell" {
				if s.shellExecutable != "" {
					parts := strings.Split(s.shellExecutable, " ")
					cmd = exec.Command(parts[0], parts[1:]...)
				} else {
					cmd = exec.Command(shell)
				}
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
			log.Printf("env %s", envVal)

			if pty != nil {
				if err := pty.Run(cmd); err != nil {
					log.Fatalf("%s", err)
				}
				s.ptySessionClientServe(channel, pty)

				s.sendStatus(channel, 0)
				s.sendSignal(channel, "TERM")

			} else {
				cmd.Stdout = channel
				cmd.Stderr = channel
				cmd.Stdin = channel
				err := cmd.Start()
				if err != nil {
					log.Printf("%s", err)
				}

				go func() {
					status, err := cmd.Process.Wait()
					if err != nil {
						log.Printf("failed to exit (%s)", err)
						cmd.Process.Kill()
					} else {
						log.Printf("command executed with exit status %s", status)
					}
					s.sendStatus(channel, uint32(status.ExitCode()))
					channel.Close()
					log.Printf("session closed")
				}()
			}

			ok = true

		case "pty-req":
			if s.disableShell {
				log.Printf("declining %s request... ", req.Type)
				req.Reply(false, nil)
				continue
			}
			// Responding 'ok' here will let the client
			// know we have a pty ready for input
			ok = true
			// allocate a terminal for this channel
			// log.Print("creating pty...")
			// Create new pty
			pty, err = rpty.New()
			if err != nil {
				log.Printf("could not start pty (%s)", err)
				return
			}
			// Parse body...
			termLen := req.Payload[3]
			termEnv := string(req.Payload[4 : termLen+4])
			w, h := parseDims(req.Payload[termLen+4:])
			pty.Resize(uint16(w), uint16(h))
			log.Printf("pty-req '%s'", termEnv)

		case "window-change":
			w, h := parseDims(req.Payload)
			pty.Resize(uint16(w), uint16(h))
			continue //no response

		case "env":
			var payload = struct{ Name, Value string }{}

			if err := ssh.Unmarshal(req.Payload, &payload); err != nil {
				log.Printf("invalid env payload: %s", req.Payload)
			}
			env[payload.Name] = payload.Value
			continue

		case "subsystem":
			var payload = struct{ Name string }{}
			if err := ssh.Unmarshal(req.Payload, &payload); err != nil {
				log.Printf("invalid env payload: %s", req.Payload)
			}
			if payload.Name == "sftp" && !s.disableSftpSubsystem {
				go s.handleSftpRequest(channel)
				ok = true
			}
		}

		if !ok {
			log.Printf("declining %s request... ", req.Type)
		}

		req.Reply(ok, nil)
	}
}

func (s *channelHandler) sendStatus(channel ssh.Channel, status uint32) {
	msg := struct {
		Status uint32
	}{
		Status: status,
	}
	if _, err := channel.SendRequest("exit-status", false, ssh.Marshal(&msg)); err != nil {
		log.Printf("failed to send exit-status: %s", err)
	}
}

func (s *channelHandler) ptySessionClientServe(channel ssh.Channel, pty rpty.Pty) {
	// Teardown session
	var once sync.Once
	close := func() {
		channel.Close()
		pty.Close()
		// log.Printf("client session closed")
	}

	// Pipe session to shell and vice-versa
	go func() {
		pty.WriteTo(channel)
		once.Do(close)
	}()

	go func() {
		pty.ReadFrom(channel)
		once.Do(close)
	}()
}

func (s *channelHandler) handleSftpRequest(channel ssh.Channel) {
	debugStream := os.Stderr
	serverOptions := []sftp.ServerOption{
		sftp.WithDebug(debugStream),
	}
	server, err := sftp.NewServer(
		channel,
		serverOptions...,
	)
	if err != nil {
		log.Fatal(err)
	}
	if err := server.Serve(); err == io.EOF {
		server.Close()
		log.Print("sftp client exited session.")
	} else if err != nil {
		log.Printf("sftp server completed with error: %s", err)
	}
}

func (s *channelHandler) sendSignal(channel ssh.Channel, signal string) {
	sig := struct {
		Signal     string
		CoreDumped bool
		Errsmg     string
		Lang       string
	}{
		Signal:     signal,
		CoreDumped: false,
		Errsmg:     "Process terminated",
		Lang:       "en-GB",
	}
	if _, err := channel.SendRequest("exit-signal", false, ssh.Marshal(&sig)); err != nil {
		log.Printf("unable to send signal: %v", err)
	}
}

func (s *channelHandler) handleChannelDirect(c ssh.NewChannel) {
	var payload = struct {
		Addr       string
		Port       uint32
		OriginAddr string
		OriginPort uint32
	}{}

	if err := ssh.Unmarshal(c.ExtraData(), &payload); err != nil {
		log.Printf("Could not unmarshal extra data: %s\n", err)

		c.Reject(ssh.Prohibited, "Bad payload")
		return
	}
	connection, requests, err := c.Accept()
	if err != nil {
		log.Printf("Could not accept channel (%s)\n", err)
		return
	}
	go ssh.DiscardRequests(requests)
	addr := fmt.Sprintf("[%s]:%d", payload.Addr, payload.Port)

	rconn, err := net.Dial("tcp", addr)
	if err != nil {
		log.Printf("Could not dial remote (%s)", err)
		connection.Close()
		return
	}

	rio.CopyConn(connection, rconn)
}

func (s *channelHandler) handleChannels() {
	// Service the incoming Channel channel.
	for newChannel := range s.chans {
		t := newChannel.ChannelType()
		switch t {
		case "session":
			go s.handleChannelSession(newChannel)
		case "direct-tcpip":
			go s.handleChannelDirect(newChannel)
		default:
			newChannel.Reject(ssh.UnknownChannelType, fmt.Sprintf("unknown channel type: %s", t))
		}
	}
}
