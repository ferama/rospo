package pipe

import "testing"

func TestRemoteParser(t *testing.T) {
	compare := func(r1 *parsedRemote, r2 *parsedRemote) bool {
		if r1.Scheme != r2.Scheme ||
			r1.Data != r2.Data {
			return false
		}
		return true
	}

	list := []string{
		"exec://python",
		"exec://python -i -u",
		":5000",
		"127.0.0.1:5000",
		"tcp://:5000",
		"tcp://127.0.0.1:5000",
	}

	expected := []parsedRemote{
		{Scheme: "exec", Data: "python"},
		{Scheme: "exec", Data: "python -i -u"},
		{Scheme: "tcp", Data: ":5000"},
		{Scheme: "tcp", Data: "127.0.0.1:5000"},
		{Scheme: "tcp", Data: ":5000"},
		{Scheme: "tcp", Data: "127.0.0.1:5000"},
	}

	for idx, s := range list {
		parsed := parseRemote(s)
		if !compare(parsed, &expected[idx]) {
			t.Fatalf("parsed: +%v expected: +%v", parsed, &expected[idx])
		}
	}
}
