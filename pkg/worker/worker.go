package worker

import "sync"

type WorkerFun func()

type Pool struct {
	jobs chan WorkerFun

	wg sync.WaitGroup
}

func NewPool(maxWorkers int) *Pool {
	pool := &Pool{
		jobs: make(chan WorkerFun, maxWorkers),
	}

	// start workers
	for range maxWorkers {
		go pool.worker()
	}

	return pool
}

func (p *Pool) worker() {
	for j := range p.jobs {
		j()
		p.wg.Done()
	}
}

func (p *Pool) Wait() {
	p.wg.Wait()
}

func (p *Pool) Stop() {
	close(p.jobs)
}

func (p *Pool) Enqueue(job WorkerFun) {
	p.wg.Add(1)
	p.jobs <- job
}
