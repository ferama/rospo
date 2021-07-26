package tun

import (
	"sync"

	"github.com/ferama/rospo/pkg/registry"
)

var (
	once     sync.Once
	instance *registry.Registry
)

// TunRegistry returns a singleton instance of Registry
func TunRegistry() *registry.Registry {
	once.Do(func() {
		instance = registry.NewRegistry()
	})

	return instance
}
