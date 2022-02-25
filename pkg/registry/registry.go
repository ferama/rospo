package registry

import (
	"errors"
	"sync"
)

// Registry is an in memory structure to hold generic map of objects
// rospo uses registries to hold maps of active tunnels
type Registry struct {
	data     map[int]interface{}
	latestID int

	mu sync.Mutex
}

// NewRegistry creates a new registry
func NewRegistry() *Registry {
	return &Registry{
		data:     make(map[int]interface{}),
		latestID: 0,
	}
}

// Add adds an items to the registry in a thread safe way
func (r *Registry) Add(t interface{}) int {
	r.mu.Lock()
	defer r.mu.Unlock()

	r.latestID++
	r.data[r.latestID] = t
	return r.latestID
}

// GetAll returns all registry contents
func (r *Registry) GetAll() map[int]interface{} {
	return r.data
}

// GetByID returns an item give its registry ID
func (r *Registry) GetByID(id int) (interface{}, error) {
	if val, ok := r.data[id]; ok {
		return val, nil
	}
	return nil, errors.New("item not found")
}

// Delete removes an item from registry
func (r *Registry) Delete(id int) error {
	if _, err := r.GetByID(id); err != nil {
		return err
	}
	r.mu.Lock()
	defer r.mu.Unlock()

	delete(r.data, id)
	return nil
}
