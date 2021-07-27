package rootapi

import (
	"net/http"

	"github.com/ferama/rospo/pkg/pipe"
	"github.com/ferama/rospo/pkg/tun"
	"github.com/gin-gonic/gin"
)

type rootRoutes struct {
	info *Info
}

func Routes(info *Info, router *gin.RouterGroup) {
	r := &rootRoutes{
		info: info,
	}

	router.GET("/info", r.getInfo)
	router.GET("/stats", r.getStats)
}

func (r *rootRoutes) getInfo(c *gin.Context) {
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
