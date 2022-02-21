package rio

import (
	"errors"
	"io"
	"sync"
)

// borrowed from the official go io package with some changes to support
// throughtput metrics
func copyBuffer(dst io.Writer, src io.Reader, wch chan int64) (err error) {
	var buf []byte
	size := 32 * 1024
	if l, ok := src.(*io.LimitedReader); ok && int64(size) > l.N {
		if l.N < 1 {
			size = 1
		} else {
			size = int(l.N)
		}
	}
	buf = make([]byte, size)
	for {
		nr, er := src.Read(buf)
		if nr > 0 {
			nw, ew := dst.Write(buf[0:nr])
			if nw < 0 || nr < nw {
				nw = 0
				if ew == nil {
					ew = errors.New("invalid write result")
				}
			}
			select {
			case wch <- int64(nw):
			default:
			}
			if ew != nil {
				err = ew
				break
			}
			if nr != nw {
				err = io.ErrShortWrite
				break
			}
		}
		if er != nil {
			if er != io.EOF {
				err = er
			}
			break
		}
	}
	return err
}

// CopyConnWithOnClose copy packets from c1 to c2 and viceversa. Calls the onClose function
// when the connection is interrupted
func CopyConnWithOnClose(
	c1 io.ReadWriteCloser,
	c2 io.ReadWriteCloser,
	metrics bool,
	onClose func()) chan int64 {

	var bw chan int64
	if metrics {
		bw = make(chan int64)
	} else {
		bw = nil
	}

	var once sync.Once
	var wg sync.WaitGroup

	connClose := func() {
		c1.Close()
		c2.Close()
		onClose()
	}

	wg.Add(2)
	go func() {
		copyBuffer(c1, c2, bw)
		once.Do(connClose)
		wg.Done()
	}()

	go func() {
		copyBuffer(c2, c1, bw)
		once.Do(connClose)
		wg.Done()
	}()

	go func() {
		wg.Wait()
		if metrics {
			close(bw)
		}
	}()

	return bw
}

// CopyConn copy packets from c1 to c2 and viceversa
func CopyConn(c1 io.ReadWriteCloser, c2 io.ReadWriteCloser) {
	CopyConnWithOnClose(c1, c2, false, func() {})
}
