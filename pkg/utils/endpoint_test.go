package utils

import "testing"

func TestEndpoint(t *testing.T) {
	val := "localhost:2222"
	e := NewEndpoint(val)
	if e.String() != val {
		t.Fail()
	}

	if (e.Host != "localhost") || (e.Port != 2222) {
		t.Fail()
	}
}
