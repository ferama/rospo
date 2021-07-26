package pipeapi

import (
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/gin-gonic/gin"
)

func Routes(sshConn *sshc.SshConnection, router *gin.RouterGroup) {
	r := &pipeRoutes{
		sshConn: sshConn,
	}
	router.GET("/", r.get)
}

type pipeRoutes struct {
	sshConn *sshc.SshConnection
}

func (r *pipeRoutes) get(c *gin.Context) {
	c.JSON(200, gin.H{
		"message": "the pipes",
	})
}
