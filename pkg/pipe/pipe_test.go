package pipe

import (
	"bufio"
	"fmt"
	"net"
	"strings"
	"testing"
	"time"
)

func startEchoService(l net.Listener) {
	for {
		conn, err := l.Accept()
		if err != nil {
			continue
		}
		go func() {
			r := bufio.NewReader(conn)
			for {
				line, err := r.ReadBytes('\n')
				if err != nil {
					return
				}
				conn.Write(line)
			}
		}()
	}
}

func TestPipe(t *testing.T) {
	l, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fail()
	}
	go startEchoService(l)
	log.Println(l.Addr().String())
	parts := strings.Split(l.Addr().String(), ":")
	port := parts[1]
	conf := &PipeConf{
		Local:  ":0",
		Remote: fmt.Sprintf(":%s", port),
	}
	pipe := NewPipe(conf, true)
	go pipe.Start()
	var pipeAddr net.Addr
	for {
		pipeAddr = pipe.GetListenerAddr()
		if pipeAddr == nil {
			time.Sleep(200 * time.Millisecond)
		} else {
			break
		}
	}
	conn, err := net.Dial("tcp", pipeAddr.String())
	if err != nil {
		t.Error(err)
	}
	_, err = conn.Write([]byte("test\n"))
	if err != nil {
		t.Error(err)
	}
	buf := make([]byte, 4)
	_, err = conn.Read(buf)
	if err != nil {
		t.Error(err)
	}
	if string(buf) != "test" {
		t.Error("assert data written is equal to data read")
	}

	if !pipe.IsStoppable() {
		t.Fail()
	}
	if !(pipe.GetActiveClientsCount() == 1) {
		t.Fail()
	}

	pipe.Stop()
	l.Close()
}

func TestPipeWithExec(t *testing.T) {
	conf := &PipeConf{
		Local:  ":0",
		Remote: "exec://echo test", // this one works on bash and windows cmd
	}
	pipe := NewPipe(conf, true)
	if pipe.GetEndpoint() != "exec://echo test" {
		t.Fail()
	}
	go pipe.Start()
	var pipeAddr net.Addr
	for {
		pipeAddr = pipe.GetListenerAddr()
		if pipeAddr == nil {
			time.Sleep(200 * time.Millisecond)
		} else {
			break
		}
	}
	conn, err := net.Dial("tcp", pipeAddr.String())
	if err != nil {
		t.Error(err)
	}
	buf := make([]byte, 4)
	_, err = conn.Read(buf)
	if err != nil {
		t.Error(err)
	}
	if string(buf) != "test" {
		t.Error("assert data written is equal to data read")
	}

	pipe.Stop()
	conn.Close()
}
