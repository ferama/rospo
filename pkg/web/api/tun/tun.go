package tunapi

import (
	"github.com/gin-gonic/gin"
)

func Routes(router *gin.RouterGroup) {
	router.GET("/", get)
}

func get(c *gin.Context) {
	c.JSON(200, gin.H{
		"message": "the tuns",
	})
}
