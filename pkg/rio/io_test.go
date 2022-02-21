package rio

import (
	"fmt"
	"log"
	"net"
	"sync"
	"testing"
)

func TestCopyConn(t *testing.T) {
	var c1WG sync.WaitGroup
	var c2WG sync.WaitGroup
	var port1 string
	var port2 string
	const payload = "test"

	c1WG.Add(1)
	c2WG.Add(1)

	go func() {
		remote, err := net.Listen("tcp", "127.0.0.1:0")
		if err != nil {
			log.Fatal(err)
		}
		_, port1, _ = net.SplitHostPort(remote.Addr().String())
		c1WG.Done()

		for {
			conn, err := remote.Accept()
			if err != nil {
				log.Fatal(err)
			}

			go func(net.Conn) {
				conn.Write([]byte(payload))
				conn.Close()
			}(conn)
		}
	}()

	go func() {
		c1WG.Wait()

		listen, err := net.Listen("tcp", "127.0.0.1:0")
		if err != nil {
			log.Fatal(err)
		}
		_, port2, _ = net.SplitHostPort(listen.Addr().String())
		c2WG.Done()
		for {
			client, err := listen.Accept()
			if err != nil {
				log.Fatal(err)
			}
			conn, _ := net.Dial("tcp", fmt.Sprintf("127.0.0.1:%s", port1))
			CopyConn(conn, client)
		}
	}()

	c1WG.Wait()
	c2WG.Wait()

	conn, _ := net.Dial("tcp", fmt.Sprintf("127.0.0.1:%s", port2))
	buf := make([]byte, len(payload))
	conn.Read(buf)
	if string(buf) != payload {
		t.Fail()
	}
}

func TestCopyConnWithOnClose(t *testing.T) {
	var c1WG sync.WaitGroup
	var c2WG sync.WaitGroup
	var port1 string
	var port2 string
	const payload = "test"

	c1WG.Add(1)
	c2WG.Add(1)

	go func() {
		remote, err := net.Listen("tcp", "127.0.0.1:0")
		if err != nil {
			log.Fatal(err)
		}
		_, port1, _ = net.SplitHostPort(remote.Addr().String())
		c1WG.Done()

		for {
			conn, err := remote.Accept()
			if err != nil {
				log.Fatal(err)
			}

			go func(net.Conn) {
				conn.Write([]byte(payload))
				conn.Close()
			}(conn)
		}
	}()

	go func() {
		c1WG.Wait()

		listen, err := net.Listen("tcp", "127.0.0.1:0")
		if err != nil {
			log.Fatal(err)
		}
		_, port2, _ = net.SplitHostPort(listen.Addr().String())
		c2WG.Done()
		for {
			client, err := listen.Accept()
			if err != nil {
				log.Fatal(err)
			}
			conn, _ := net.Dial("tcp", fmt.Sprintf("127.0.0.1:%s", port1))
			bw := CopyConnWithOnClose(conn, client, true, func() {})
			var totalBytes int64
			totalBytes = 0
			for w := range bw {
				totalBytes += w
			}
			if int(totalBytes) != len(payload) {
				t.Fail()
			}
		}
	}()

	c1WG.Wait()
	c2WG.Wait()

	conn, _ := net.Dial("tcp", fmt.Sprintf("127.0.0.1:%s", port2))
	buf := make([]byte, len(payload))
	conn.Read(buf)
	if string(buf) != payload {
		t.Fail()
	}
}
