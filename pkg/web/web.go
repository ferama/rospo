package web

import (
	"time"

	"github.com/ferama/rospo/pkg/sshc"
	rootapi "github.com/ferama/rospo/pkg/web/api/root"
	tunapi "github.com/ferama/rospo/pkg/web/api/tun"
	"github.com/gin-contrib/cors"
	"github.com/gin-gonic/gin"
)

// StartServer start the rospo web server. The webserver
// exposes rospo apis and a nice ui at the /
func StartServer(isDev bool,
	sshConn *sshc.SshConnection,
	conf *WebConf,
	info *rootapi.Info) {

	if !isDev {
		gin.SetMode(gin.ReleaseMode)
	}
	r := gin.Default()

	r.Use(cors.New(cors.Config{
		AllowOrigins:     []string{"*"},
		AllowMethods:     []string{"*"},
		AllowHeaders:     []string{"Content-Type, Origin"},
		ExposeHeaders:    []string{"Content-Length"},
		AllowCredentials: true,
		MaxAge:           12 * time.Hour,
	}))

	rootapi.Routes(info, sshConn, r.Group("/api"))
	tunapi.Routes(sshConn, r.Group("/api/tuns"))

	r.Run(conf.ListenAddress)
}
