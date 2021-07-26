package tunapi

import (
	"strconv"

	"github.com/ferama/rospo/pkg/sshc"
	"github.com/ferama/rospo/pkg/tun"
	"github.com/gin-gonic/gin"
)

func Routes(sshConn *sshc.SshConnection, router *gin.RouterGroup) {
	r := &tunRoutes{
		sshConn: sshConn,
	}
	router.GET("/", r.get)
	router.DELETE("/:tun-id", r.delete)
	router.POST("/", r.post)
}

type tunRoutes struct {
	sshConn *sshc.SshConnection
}

func (r *tunRoutes) get(c *gin.Context) {
	data := tun.TunRegistry().GetAll()
	var res []item
	for id, val := range data {
		tunnel := val.(*tun.Tunnel)
		addr, _ := tunnel.GetListenerAddr()
		res = append(res, item{
			ID:   id,
			Addr: addr,
		})
	}
	c.JSON(200, res)
}

func (r *tunRoutes) delete(c *gin.Context) {
	tunId, err := strconv.Atoi(c.Param("tun-id"))

	if err != nil {
		c.JSON(404, gin.H{
			"error": err.Error(),
		})
		return
	}

	data, err := tun.TunRegistry().GetByID(tunId)

	if err != nil {
		c.JSON(404, gin.H{
			"error": err.Error(),
		})
		return
	}
	tunnel := data.(*tun.Tunnel)
	tunnel.Stop()
	addr, _ := tunnel.GetListenerAddr()
	c.JSON(200, gin.H{
		"addr": addr,
	})
}

func (r *tunRoutes) post(c *gin.Context) {
	// TODO
}
