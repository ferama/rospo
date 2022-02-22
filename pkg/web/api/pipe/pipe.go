package pipeapi

import (
	"net/http"
	"strconv"

	"github.com/ferama/rospo/pkg/pipe"
	"github.com/ferama/rospo/pkg/utils"
	"github.com/gin-gonic/gin"
)

// Routes setup pipe related api routes
func Routes(router *gin.RouterGroup) {
	r := &pipeRoutes{}

	router.GET("", r.get)
	router.GET(":pipe-id", r.get)
	router.DELETE(":pipe-id", r.delete)
	router.POST("", r.post)
}

type pipeRoutes struct {
}

func (r *pipeRoutes) get(c *gin.Context) {
	if c.Param("pipe-id") == "" {
		var res []pipeResponseItem
		data := pipe.PipeRegistry().GetAll()
		for id, val := range data {
			pipeItem := val.(*pipe.Pipe)
			addr := pipeItem.GetListenerAddr()
			res = append(res, pipeResponseItem{
				ID:               id,
				Listener:         addr,
				IsStoppable:      pipeItem.IsStoppable(),
				Endpoint:         pipeItem.GetEndpoint(),
				ClientsCount:     pipeItem.GetActiveClientsCount(),
				Throughput:       pipeItem.GetCurrentBytesPerSecond(),
				ThroughputString: utils.ByteCountSI(pipeItem.GetCurrentBytesPerSecond()) + "/s",
			})
		}
		c.JSON(http.StatusOK, res)

	} else {
		tunId, err := strconv.Atoi(c.Param("pipe-id"))
		if err != nil {
			c.JSON(http.StatusNotFound, gin.H{
				"error": err.Error(),
			})
			return
		}
		val, err := pipe.PipeRegistry().GetByID(tunId)
		if err != nil {
			c.JSON(http.StatusNotFound, gin.H{
				"error": err.Error(),
			})
			return
		}
		pipeItem := val.(*pipe.Pipe)
		addr := pipeItem.GetListenerAddr()
		c.JSON(http.StatusOK, pipeResponseItem{
			ID:               tunId,
			Listener:         addr,
			IsStoppable:      pipeItem.IsStoppable(),
			Endpoint:         pipeItem.GetEndpoint(),
			ClientsCount:     pipeItem.GetActiveClientsCount(),
			Throughput:       pipeItem.GetCurrentBytesPerSecond(),
			ThroughputString: utils.ByteCountSI(pipeItem.GetCurrentBytesPerSecond()) + "/s",
		})
	}
}

func (r *pipeRoutes) delete(c *gin.Context) {
	pipeId, err := strconv.Atoi(c.Param("pipe-id"))

	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{
			"error": err.Error(),
		})
		return
	}

	data, err := pipe.PipeRegistry().GetByID(pipeId)

	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{
			"error": err.Error(),
		})
		return
	}
	pipeItem := data.(*pipe.Pipe)
	pipeItem.Stop()
	c.JSON(http.StatusOK, gin.H{})
}

// Example curl:
// curl -X POST -H "Content-Type: application/json" --data '{"remote": "rpi:22", "local": ":3322"}' http://localhost:8090/api/pipes/
func (r *pipeRoutes) post(c *gin.Context) {
	var conf pipe.PipeConf
	if err := c.BindJSON(&conf); err != nil {
		c.JSON(http.StatusNotFound, gin.H{
			"error": err.Error(),
		})
		return
	}
	pipeItem := pipe.NewPipe(&conf, true)
	go pipeItem.Start()
	c.JSON(http.StatusOK, gin.H{})
}
