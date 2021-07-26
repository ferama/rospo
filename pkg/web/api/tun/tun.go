package tunapi

import (
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/gin-gonic/gin"
)

func Routes(sshConn *sshc.SshConnection, router *gin.RouterGroup) {
	r := &tunRoutes{
		sshConn: sshConn,
	}
	router.GET("/", r.get)
}

type tunRoutes struct {
	sshConn *sshc.SshConnection
}

func (r *tunRoutes) get(c *gin.Context) {
	// TODO: get data from TunRegistry. Map the data to the tun conf and return it as json
	c.JSON(200, gin.H{
		"message": "the tuns",
	})
}
