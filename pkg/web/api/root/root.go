package rootapi

import (
	"net/http"

	"github.com/ferama/rospo/pkg/pipe"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/ferama/rospo/pkg/tun"
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

	router.GET("/info", r.getInfo)
	router.GET("/stats", r.getStats)
}

func (r *rootRoutes) getInfo(c *gin.Context) {
	r.info.SshClientConnectionStatus = r.sshConn.ConnectionStatus
	c.JSON(http.StatusOK, r.info)
}

func (r *rootRoutes) getStats(c *gin.Context) {
	t := tun.TunRegistry().GetAll()
	tunnelClientsCount := 0
	for _, val := range t {
		tunnel := val.(*tun.Tunnel)
		tunnelClientsCount += tunnel.GetActiveClientsCount()
	}

	p := pipe.PipeRegistry().GetAll()
	pipeClientsCount := 0
	for _, val := range p {
		pipeI := val.(*pipe.Pipe)
		pipeClientsCount += pipeI.GetActiveClientsCount()
	}

	response := &statsResponse{
		CountTunnels:        len(t),
		CountTunnelsClients: tunnelClientsCount,

		CountPipes:        len(p),
		CountPipesClients: pipeClientsCount,
	}
	c.JSON(http.StatusOK, response)
}
