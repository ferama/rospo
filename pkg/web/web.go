package web

import (
	"io/fs"
	"net/http"
	"time"

	"github.com/ferama/rospo/pkg/sshc"
	pipeapi "github.com/ferama/rospo/pkg/web/api/pipe"
	rootapi "github.com/ferama/rospo/pkg/web/api/root"
	tunapi "github.com/ferama/rospo/pkg/web/api/tun"
	"github.com/ferama/rospo/pkg/web/ui"
	"github.com/gin-contrib/cors"
	"github.com/gin-gonic/gin"
)

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

	rootapi.Routes(info, r.Group("/api"))
	pipeapi.Routes(r.Group("/api/pipes"))
	tunapi.Routes(sshConn, r.Group("/api/tuns"))

	// static files custom middleware
	// use the "build" dir (the webpack target) as static root
	fsRoot, _ := fs.Sub(ui.StaticFiles, "build")
	fileserver := http.FileServer(http.FS(fsRoot))
	r.Use(func(c *gin.Context) {
		fileserver.ServeHTTP(c.Writer, c.Request)
		c.Abort()
	})

	r.Run(conf.ListenAddress)
}
