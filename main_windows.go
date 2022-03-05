package main

import (
	"sync"

	"github.com/ferama/rospo/cmd"
	"github.com/judwhite/go-svc"
	wsvc "golang.org/x/sys/windows/svc"
)

// rospo implements svc.Service
type rospo struct {
	wg   sync.WaitGroup
	quit chan bool
}

// Init initialize the rospo service
func (r *rospo) Init(env svc.Environment) error {
	return nil
}

// Start starts the rospo windows service
func (r *rospo) Start() error {
	// The Start method must not block, or Windows may assume your service failed
	// to start. Launch a Goroutine here to do something interesting/blocking.
	r.wg.Add(1)
	go func() {
		go cmd.Execute()
		<-r.quit
		r.wg.Done()
	}()

	return nil
}

// Stop shutdown the windows service
func (r *rospo) Stop() error {
	// The Stop method is invoked by stopping the Windows service, or by pressing Ctrl+C on the console.
	// This method may block, but it's a good idea to finish quickly or your process may be killed by
	// Windows during a shutdown/reboot. As a general rule you shouldn't rely on graceful shutdown.
	close(r.quit)
	r.wg.Wait()
	return nil
}

func main() {
	isWindowsService, err := wsvc.IsWindowsService()
	if err != nil {
		panic(err)
	}
	if isWindowsService {
		prg := &rospo{
			quit: make(chan bool),
		}

		// Call svc.Run to start rospo service
		if err := svc.Run(prg); err != nil {
			panic(err)
		}
	} else {
		// run as usuale if we are not running as windows service
		cmd.Execute()
	}
}
