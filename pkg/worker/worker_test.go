package worker

import (
	"log"
	"testing"
	"time"
)

func TestWorkerPool(t *testing.T) {
	wp := NewPool(15)
	for range 30 {
		wp.Enqueue(func() {
			time.Sleep(2 * time.Second)
			log.Println()
		})
	}

	wp.Wait()
}
