package sshd

import (
	"encoding/binary"
	"fmt"
	"io"
	"net"
	"os"
	"os/exec"
	"runtime"
	"runtime/debug"
	"strings"
	"sync"

	"github.com/ferama/rospo/pkg/rio"
	"github.com/ferama/rospo/pkg/rpty"
	"github.com/ferama/rospo/pkg/utils"
	"github.com/pkg/sftp"
	"golang.org/x/crypto/ssh"
)

// ptyRequestMsg is the payload of a "pty-req" channel request (RFC 4254 6.2)
type ptyRequestMsg struct {
	Term     string
	Columns  uint32
	Rows     uint32
	Width    uint32
	Height   uint32
	Modelist string
}

// parseDims extracts two uint32s from the provided buffer.
func parseDims(b []byte) (uint32, uint32, error) {
	if len(b) < 8 {
		return 0, 0, fmt.Errorf("payload too short: %d bytes", len(b))
	}
	w := binary.BigEndian.Uint32(b)
	h := binary.BigEndian.Uint32(b[4:])
	return w, h, nil
}

// recoverAndClose prevents a panic in a per channel goroutine from
// taking down the whole daemon. Must be called with defer
func recoverAndClose(where string, c io.Closer) {
	if r := recover(); r != nil {
		log.Printf("recovered panic in %s: %v\n%s", where, r, debug.Stack())
		if c != nil {
			c.Close()
		}
	}
}

type channelHandler struct {
	server  *sshServer
	sshConn *ssh.ServerConn

	chans <-chan ssh.NewChannel
}

func newChannelHandler(
	server *sshServer,
	sshConn *ssh.ServerConn,
	chans <-chan ssh.NewChannel,
) *channelHandler {

	return &channelHandler{
		server:  server,
		sshConn: sshConn,
		chans:   chans,
	}

}

func (s *channelHandler) handleShellExecRequest(
	pty rpty.Pty,
	env map[string]string,
	channel ssh.Channel,
	req *ssh.Request) bool {

	var shell string

	if s.server.shellExecutable == "" {
		usr := utils.CurrentUser()
		shell = utils.GetUserDefaultShell(usr.Username)
	} else {
		shell = s.server.shellExecutable
	}

	if s.server.disableShell {
		return false
	}
	var cmd *exec.Cmd

	if req.Type == "shell" {
		if s.server.shellExecutable != "" {
			parts := strings.Split(s.server.shellExecutable, " ")
			cmd = exec.Command(parts[0], parts[1:]...)
		} else {
			cmd = exec.Command(shell)
		}
	} else {
		var payload = struct{ Value string }{}
		ssh.Unmarshal(req.Payload, &payload)
		command := payload.Value
		cmd = exec.Command(shell, []string{"-c", command}...)
		if runtime.GOOS == "windows" {
			command = strings.Replace(command, "powershell.exe", "", 1)
			command = strings.Replace(command, "powershell", "", 1)
			cmd = exec.Command(shell, command)
		}
	}

	envVal := make([]string, 0, len(env))
	for k, v := range env {
		envVal = append(envVal, fmt.Sprintf("%s=%s", k, v))
	}

	usr := utils.CurrentUser()

	// export TERM
	term := os.Getenv("TERM")
	if term == "" {
		term = "xterm"
	}
	envVal = append(envVal, fmt.Sprintf("TERM=%s", term))

	// export HOME
	envVal = append(envVal, fmt.Sprintf("HOME=%s", usr.HomeDir))

	// export USER
	envVal = append(envVal, fmt.Sprintf("USER=%s", usr.Username))
	envVal = append(envVal, fmt.Sprintf("LOGNAME=%s", usr.Username))

	// export PATH
	path := os.Getenv("PATH")
	if path == "" {
		if runtime.GOOS == "windows" {
			path = `C:\Windows\system32;C:\Windows;C:\Windows\System32\Wbem`
		} else {
			path = "/usr/local/bin:/usr/bin:/bin:/usr/local/sbin:/usr/sbin:/sbin"
		}
	}
	envVal = append(envVal, fmt.Sprintf("PATH=%s", path))

	// export SHELL
	envVal = append(envVal, fmt.Sprintf("SHELL=%s", shell))

	cmd.Env = envVal
	cmd.Dir = usr.HomeDir

	if pty != nil {
		if err := pty.Run(cmd); err != nil {
			log.Printf("could not run command on pty (%s)", err)
			return false
		}
		s.ptySessionClientServe(channel, pty)

		s.sendStatus(channel, 0)
		s.sendSignal(channel, "TERM")

	} else {
		cmd.Stdout = channel
		cmd.Stderr = channel
		cmd.Stdin = channel
		if err := cmd.Start(); err != nil {
			log.Printf("could not start command (%s)", err)
			return false
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

	return true
}

func (s *channelHandler) handlePtyRequest(req *ssh.Request) (rpty.Pty, error) {
	if s.server.disableShell {
		return nil, nil
	}

	var payload ptyRequestMsg
	if err := ssh.Unmarshal(req.Payload, &payload); err != nil {
		log.Printf("invalid pty-req payload (%s)", err)
		return nil, nil
	}

	// allocate a terminal for this channel
	pty, err := rpty.New()
	if err != nil {
		return nil, err
	}
	pty.Resize(uint16(payload.Columns), uint16(payload.Rows))

	log.Printf("pty-req '%s'", payload.Term)
	return pty, nil
}

func (s *channelHandler) serveChannelSession(c ssh.NewChannel) {
	channel, requests, err := c.Accept()
	if err != nil {
		log.Printf("could not accept channel (%s)", err)
		return
	}
	defer recoverAndClose("session channel", channel)

	var pty rpty.Pty
	env := map[string]string{}

	for req := range requests {
		ok := false
		switch req.Type {
		case "shell", "exec":
			ok = s.handleShellExecRequest(pty, env, channel, req)

		case "pty-req":
			if pty != nil {
				// only one pty per session
				break
			}
			pty, err = s.handlePtyRequest(req)
			if err != nil {
				log.Printf("could not start pty (%s)", err)
				req.Reply(false, nil)
				channel.Close()
				return
			}
			ok = pty != nil

		case "window-change":
			if pty == nil {
				break
			}
			w, h, err := parseDims(req.Payload)
			if err != nil {
				log.Printf("invalid window-change payload (%s)", err)
				break
			}
			pty.Resize(uint16(w), uint16(h))
			ok = true

		case "env":
			var payload = struct{ Name, Value string }{}

			if err := ssh.Unmarshal(req.Payload, &payload); err != nil {
				log.Printf("invalid env payload: %s", req.Payload)
			}
			log.Printf("setenv: %s=%s", payload.Name, payload.Value)

			env[payload.Name] = payload.Value
			ok = true

		case "subsystem":
			var payload = struct{ Name string }{}
			if err := ssh.Unmarshal(req.Payload, &payload); err != nil {
				log.Printf("invalid payload: %s", req.Payload)
			}
			if payload.Name == "sftp" && !s.server.disableSftpSubsystem {
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
	defer recoverAndClose("sftp subsystem", channel)

	debugStream := os.Stderr
	serverOptions := []sftp.ServerOption{
		sftp.WithDebug(debugStream),
	}
	server, err := sftp.NewServer(
		channel,
		serverOptions...,
	)
	if err != nil {
		log.Printf("could not start sftp server (%s)", err)
		channel.Close()
		return
	}
	if err := server.Serve(); err != nil {
		if err != io.EOF {
			log.Printf("sftp server completed with error: %s", err)
		}
	}
	server.Close()
	log.Print("sftp client exited session.")
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
	defer recoverAndClose("direct-tcpip channel", connection)
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
			// shell, exec and sft subsystem
			go s.serveChannelSession(newChannel)
		case "direct-tcpip":
			if s.server.disableTunnelling {
				newChannel.Reject(ssh.Prohibited, "tunnelling is disabled")
				continue
			}
			// used by forward requests
			go s.handleChannelDirect(newChannel)
		default:
			newChannel.Reject(ssh.UnknownChannelType, fmt.Sprintf("unknown channel type: %s", t))
		}
	}
}
