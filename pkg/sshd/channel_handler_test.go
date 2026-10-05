package sshd

import (
	"testing"

	"golang.org/x/crypto/ssh"
)

func TestParseDims(t *testing.T) {
	for _, b := range [][]byte{{}, {0, 0, 0}, {0, 0, 0, 0, 0, 0, 0}} {
		if _, _, err := parseDims(b); err == nil {
			t.Errorf("expected error for %d bytes payload", len(b))
		}
	}

	w, h, err := parseDims([]byte{0, 0, 0, 80, 0, 0, 0, 24})
	if err != nil {
		t.Fatal(err)
	}
	if w != 80 || h != 24 {
		t.Fatalf("got %dx%d, expected 80x24", w, h)
	}
}

func TestMalformedChannelRequests(t *testing.T) {
	_, sshdPort := startD(false)
	conn := getSSHConn(sshdPort)

	validPty := ssh.Marshal(ptyRequestMsg{Term: "xterm", Columns: 80, Rows: 24})

	cases := []struct {
		name    string
		reqs    []string
		payload [][]byte
		// expected reply for the last request
		expected bool
	}{
		{"window-change before pty-req", []string{"window-change"}, [][]byte{{0, 0, 0, 80, 0, 0, 0, 24}}, false},
		{"empty window-change", []string{"pty-req", "window-change"}, [][]byte{validPty, {}}, false},
		{"short window-change", []string{"pty-req", "window-change"}, [][]byte{validPty, {0, 0, 0}}, false},
		{"empty pty-req", []string{"pty-req"}, [][]byte{{}}, false},
		{"short pty-req", []string{"pty-req"}, [][]byte{{0, 0, 0}}, false},
		// a term length byte of 0xfc makes termLen+4 wrap around to 0
		{"wrapping pty-req term length", []string{"pty-req"}, [][]byte{{0, 0, 0, 0xfc, 'x'}}, false},
		{"oversized pty-req term length", []string{"pty-req"}, [][]byte{{0, 0, 0, 0x7f, 'x'}}, false},
		{"duplicate pty-req", []string{"pty-req", "pty-req"}, [][]byte{validPty, validPty}, false},
		{"valid pty-req and window-change", []string{"pty-req", "window-change"}, [][]byte{validPty, {0, 0, 0, 100, 0, 0, 0, 40}}, true},
	}

	for _, c := range cases {
		ch, _, err := conn.Client.OpenChannel("session", nil)
		if err != nil {
			t.Fatalf("%s: %s", c.name, err)
		}
		var ok bool
		for i, r := range c.reqs {
			ok, err = ch.SendRequest(r, true, c.payload[i])
			if err != nil {
				t.Fatalf("%s: %s", c.name, err)
			}
		}
		if ok != c.expected {
			t.Errorf("%s: got reply %v, expected %v", c.name, ok, c.expected)
		}
		ch.Close()
	}

	// the daemon must still be serving new sessions
	sess, err := conn.Client.NewSession()
	if err != nil {
		t.Fatal(err)
	}
	defer sess.Close()
	if err := sess.RequestPty("xterm", 24, 80, ssh.TerminalModes{}); err != nil {
		t.Fatal(err)
	}
}
