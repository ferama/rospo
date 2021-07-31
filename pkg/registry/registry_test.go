package registry

import (
	"sync"
	"testing"
)

func TestConcurrentAdd(t *testing.T) {
	r := NewRegistry()
	type data struct {
		n int
	}
	var wg sync.WaitGroup
	const citems = 3
	c := citems
	wg.Add(c)
	for c > 0 {
		go func(val int) {
			r.Add(data{
				n: val,
			})
			wg.Done()
		}(c)
		c--
	}
	wg.Wait()
	r.Delete(1)
	res := r.GetAll()
	if len(res) != citems-1 {
		t.Fatalf("registry content len is wrong. Should be %d is %d", citems-1, len(res))
	}
}

func TestGetDelete(t *testing.T) {
	r := NewRegistry()
	type data struct {
		n int
	}
	r.Add(data{
		n: 1,
	})
	_, err := r.GetByID(1)
	if err != nil {
		t.Fatal(err)
	}

	_, err = r.GetByID(2)
	if err == nil {
		t.Fatal("should return err")
	}

	err = r.Delete(2)
	if err == nil {
		t.Fatal("should return err")
	}
	err = r.Delete(1)
	if err != nil {
		t.Fatal("should not return err")
	}
}
