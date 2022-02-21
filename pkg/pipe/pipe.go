package pipe

import (
	"net"
	"os/exec"
	"runtime"
	"sync"
	"time"

	"github.com/ferama/rospo/pkg/logger"
	"github.com/ferama/rospo/pkg/rio"
)

var log = logger.NewLogger("[PIPE] ", logger.Red)

// Pipe struct definition
type Pipe struct {
	local  string
	remote string

	// the pipe connection listener
	listener net.Listener

	// indicate if the pipe should be terminated
	terminate chan bool
	stoppable bool

	registryID int

	// holds all active pipe clients
	clientsMap   map[string]net.Conn
	clientsMapMU sync.Mutex

	// holds all active subprocesses
	processes   map[int]*exec.Cmd
	processesMU sync.Mutex

	listenerMU sync.RWMutex

	// metrics related
	currentBytes          int64
	currentBytesPerSecond int64
	metricsMU             sync.RWMutex
	metricsSamplerCloser  chan bool
}

// NewPipe creates a Pipe object
func NewPipe(conf *PipeConf, stoppable bool) *Pipe {
	pipe := &Pipe{
		local:      conf.Local,
		remote:     conf.Remote,
		terminate:  make(chan bool),
		stoppable:  stoppable,
		clientsMap: make(map[string]net.Conn),
		processes:  make(map[int]*exec.Cmd),

		currentBytes:          0,
		currentBytesPerSecond: 0,
		metricsSamplerCloser:  make(chan bool),
	}
	return pipe
}

// GetsCurrentBytesPerSecond return the current tunnel throughput
func (p *Pipe) GetCurrentBytesPerSecond() int64 {
	p.metricsMU.RLock()
	defer p.metricsMU.RUnlock()
	return p.currentBytesPerSecond
}

// GetListenerAddr returns the pipe listener network address
func (p *Pipe) GetListenerAddr() net.Addr {
	p.listenerMU.RLock()
	defer p.listenerMU.RUnlock()

	if p.listener != nil {
		return p.listener.Addr()
	}
	return nil
}

// GetEndpoint returns the pipe remote endpoint
// This is actually used from pipe api routes
func (p *Pipe) GetEndpoint() string {
	return p.remote
}

// GetActiveClientsCount returns how many clients are actually using the pipe
func (p *Pipe) GetActiveClientsCount() int {
	p.clientsMapMU.Lock()
	defer p.clientsMapMU.Unlock()

	return len(p.clientsMap)
}

// Start the pipe. It basically copy all the tcp packets incoming to the
// local endpoint into the remote endpoint
func (p *Pipe) Start() {
	listener, err := net.Listen("tcp", p.local)
	if err != nil {
		log.Printf("listening on %s error.\n", err)
		return
	}
	p.listenerMU.Lock()
	p.listener = listener
	p.listenerMU.Unlock()

	p.registryID = PipeRegistry().Add(p)
	go p.metricsSampler()

	log.Printf("listening on %s\n", p.local)
	for {
		select {
		case <-p.terminate:
			return
		default:
			client, err := listener.Accept()
			if err != nil {
				log.Println("disconnected")
				break
			}
			p.clientsMapMU.Lock()
			p.clientsMap[client.RemoteAddr().String()] = client
			p.clientsMapMU.Unlock()
			go p.handleRemote(client)
		}

	}
}

func (p *Pipe) metricsSampler() {
	for {
		select {
		case <-p.metricsSamplerCloser:
			return
		case <-time.After(5 * time.Second):
			p.metricsMU.Lock()
			p.currentBytesPerSecond = p.currentBytes / 5
			p.currentBytes = 0
			p.metricsMU.Unlock()
			// log.Printf("pipe: [%d] - %s/s", p.registryID, utils.ByteCountSI(p.currentBytesPerSecond))
		}
	}
}

func (p *Pipe) handleRemote(client net.Conn) {
	parsed := parseRemote(p.remote)
	switch scheme := parsed.Scheme; scheme {
	case "exec":
		p.handleExecRemote(client, parsed.Data)
	case "tcp":
		p.handleTcpRemote(client)
	}
}

func (p *Pipe) handleExecRemote(client net.Conn, cmdline string) {
	var cmd *exec.Cmd
	if runtime.GOOS != "windows" {
		cmd = exec.Command("sh", "-c", cmdline)
	} else {
		cmd = exec.Command("cmd", "/C", cmdline)
	}
	stdin, err := cmd.StdinPipe()
	if err != nil {
		log.Println(err)
		client.Close()
		return
	}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		log.Println(err)
		client.Close()
		return
	}
	cmd.Stderr = cmd.Stdout

	cmd.Start()

	p.processesMU.Lock()
	p.processes[cmd.Process.Pid] = cmd
	p.processesMU.Unlock()

	var once sync.Once
	var wg sync.WaitGroup
	// byteswritten
	bw := make(chan int64)

	connClose := func() {
		p.processesMU.Lock()
		delete(p.processes, cmd.Process.Pid)
		p.processesMU.Unlock()

		cmd.Process.Kill()

		client.Close()
		p.clientsMapMU.Lock()
		delete(p.clientsMap, client.RemoteAddr().String())
		p.clientsMapMU.Unlock()

	}

	wg.Add(2)
	go func() {
		rio.CopyBuffer(stdin, client, bw)
		once.Do(connClose)
		wg.Done()
	}()
	go func() {
		rio.CopyBuffer(client, stdout, bw)
		once.Do(connClose)
		wg.Done()
	}()

	// waits for the bidirectional copy routines to terminate
	go func() {
		wg.Wait()
		close(bw)
	}()

	// consumes messages from bytewritten channel and stores value
	go func() {
		for w := range bw {
			p.metricsMU.Lock()
			p.currentBytes += w
			p.metricsMU.Unlock()
		}
	}()
}

func (p *Pipe) handleTcpRemote(client net.Conn) {
	conn, err := net.Dial("tcp", p.remote)
	if err != nil {
		log.Println("remote connection refused")
		client.Close()
		return
	}
	byteswrittench := rio.CopyConnWithOnClose(client, conn, true, func() {
		p.clientsMapMU.Lock()
		delete(p.clientsMap, client.RemoteAddr().String())
		p.clientsMapMU.Unlock()
	})
	go func() {
		for w := range byteswrittench {
			p.metricsMU.Lock()
			p.currentBytes += w
			p.metricsMU.Unlock()
		}
	}()
}

// IsStoppable return true if the pipe can be stopped calling the Stop
// method. False if not
func (p *Pipe) IsStoppable() bool {
	return p.stoppable
}

// Stop closes the pipe
func (p *Pipe) Stop() {
	if !p.stoppable {
		return
	}
	close(p.metricsSamplerCloser)
	PipeRegistry().Delete(p.registryID)
	close(p.terminate)
	go func() {
		p.listenerMU.RLock()
		if p.listener != nil {
			p.listener.Close()
		}
		p.listenerMU.RUnlock()

		// close all clients connections
		p.clientsMapMU.Lock()
		for k, v := range p.clientsMap {
			v.Close()
			delete(p.clientsMap, k)
		}
		p.clientsMapMU.Unlock()

		p.processesMU.Lock()
		for pid, v := range p.processes {
			delete(p.processes, pid)
			v.Process.Kill()
		}

		p.processesMU.Unlock()
	}()
}
