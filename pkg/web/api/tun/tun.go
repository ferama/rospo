package tunapi

import (
	"net/http"
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
	router.GET("/:tun-id", r.get)
	router.DELETE("/:tun-id", r.delete)
	router.POST("/", r.post)
}

type tunRoutes struct {
	sshConn *sshc.SshConnection
}

func (r *tunRoutes) get(c *gin.Context) {
	if c.Param("tun-id") == "" {
		var res []responseItem
		data := tun.TunRegistry().GetAll()
		for id, val := range data {
			tunnel := val.(*tun.Tunnel)
			addr := tunnel.GetListenerAddr()
			res = append(res, responseItem{
				ID:              id,
				Listener:        addr,
				IsListenerLocal: tunnel.GetIsListenerLocal(),
				Endpoint:        tunnel.GetEndpoint(),
				ClientsCount:    tunnel.GetActiveClientsCount(),
			})
		}
		c.JSON(http.StatusOK, res)

	} else {
		tunId, err := strconv.Atoi(c.Param("tun-id"))
		if err != nil {
			c.JSON(http.StatusNotFound, gin.H{
				"error": err.Error(),
			})
			return
		}
		val, err := tun.TunRegistry().GetByID(tunId)
		if err != nil {
			c.JSON(http.StatusNotFound, gin.H{
				"error": err.Error(),
			})
			return
		}
		tunnel := val.(*tun.Tunnel)
		addr := tunnel.GetListenerAddr()
		c.JSON(http.StatusOK, responseItem{
			ID:              tunId,
			Listener:        addr,
			IsListenerLocal: tunnel.GetIsListenerLocal(),
			Endpoint:        tunnel.GetEndpoint(),
			ClientsCount:    tunnel.GetActiveClientsCount(),
		})
	}
}

func (r *tunRoutes) delete(c *gin.Context) {
	tunId, err := strconv.Atoi(c.Param("tun-id"))

	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{
			"error": err.Error(),
		})
		return
	}

	data, err := tun.TunRegistry().GetByID(tunId)

	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{
			"error": err.Error(),
		})
		return
	}
	tunnel := data.(*tun.Tunnel)
	tunnel.Stop()
	addr := tunnel.GetListenerAddr()
	c.JSON(http.StatusOK, gin.H{
		"Listener": addr,
	})
}

// Example curl:
// curl -X POST -H "Content-Type: application/json" --data '{"remote": ":5005", "local": ":5000", "forward": false}' http://localhost:8090/api/tuns/
func (r *tunRoutes) post(c *gin.Context) {
	var conf tun.TunnelConf
	if err := c.BindJSON(&conf); err != nil {
		c.JSON(http.StatusNotFound, gin.H{
			"error": err.Error(),
		})
		return
	}
	tunnel := tun.NewTunnel(r.sshConn, &conf)
	go tunnel.Start()
	addr := tunnel.GetListenerAddr()
	c.JSON(http.StatusOK, gin.H{
		"Listener": addr,
	})
}
