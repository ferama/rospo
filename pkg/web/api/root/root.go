package rootapi

import (
	"net/http"
	"runtime"

	"github.com/ferama/rospo/pkg/sshc"
	"github.com/ferama/rospo/pkg/tun"
	"github.com/ferama/rospo/pkg/utils"
	"github.com/gin-gonic/gin"
)

type rootRoutes struct {
	info    *Info
	sshConn *sshc.SshConnection
}

// Routes setup the root api routes
func Routes(info *Info, sshConn *sshc.SshConnection, router *gin.RouterGroup) {
	r := &rootRoutes{
		info:    info,
		sshConn: sshConn,
	}

	router.GET("info", r.getInfo)
	router.GET("stats", r.getStats)
}

func (r *rootRoutes) getInfo(c *gin.Context) {
	r.info.SshClientConnectionStatus = r.sshConn.GetConnectionStatus()
	c.JSON(http.StatusOK, r.info)
}

func (r *rootRoutes) getStats(c *gin.Context) {
	t := tun.TunRegistry().GetAll()
	tunnelClientsCount := 0
	var tunnelThroughput int64
	tunnelThroughput = 0
	for _, val := range t {
		tunnel := val.(*tun.Tunnel)
		tunnelClientsCount += tunnel.GetActiveClientsCount()
		tunnelThroughput += tunnel.GetCurrentBytesPerSecond()
	}

	memStats := new(runtime.MemStats)
	runtime.ReadMemStats(memStats)
	response := &struct {
		CountTunnels        int
		CountTunnelsClients int

		// runtime stats
		NumGoroutine int
		MemTotal     uint64

		TotalPipeThroughput         int64
		TotalPipeThroughputString   string
		TotalTunnelThroughput       int64
		TotalTunnelThroughputString string
	}{
		CountTunnels:        len(t),
		CountTunnelsClients: tunnelClientsCount,
		NumGoroutine:        runtime.NumGoroutine(),
		MemTotal:            memStats.HeapInuse + memStats.StackInuse + memStats.MSpanInuse + memStats.MCacheInuse,

		TotalTunnelThroughput:       tunnelThroughput,
		TotalTunnelThroughputString: utils.ByteCountSI(tunnelThroughput),
	}

	c.JSON(http.StatusOK, response)
}
