package rootapi

import (
	"net/http"

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
}

func (r *rootRoutes) getInfo(c *gin.Context) {
	c.JSON(http.StatusOK, r.info)
}
