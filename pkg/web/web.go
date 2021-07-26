package web

import (
	pipeapi "github.com/ferama/rospo/pkg/web/api/pipe"
	tunapi "github.com/ferama/rospo/pkg/web/api/tun"
	"github.com/gin-gonic/gin"
)

func StartServer(isDev bool, conf *WebConf) {
	if !isDev {
		gin.SetMode(gin.ReleaseMode)
	}
	r := gin.Default()
	r.GET("/ping", func(c *gin.Context) {
		c.JSON(200, gin.H{
			"message": "pong",
		})
	})

	pipeapi.Routes(r.Group("/api/pipes"))
	tunapi.Routes(r.Group("/api/tuns"))

	// listen and serve on 0.0.0.0:8080 (for windows "localhost:8080")
	r.Run()
}
