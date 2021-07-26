package pipe

import (
	"sync"

	"github.com/ferama/rospo/pkg/registry"
)

var (
	once     sync.Once
	instance *registry.Registry
)

// PipeRegistry returns a singleton instance of Registry
func PipeRegistry() *registry.Registry {
	once.Do(func() {
		instance = registry.NewRegistry()
	})

	return instance
}
