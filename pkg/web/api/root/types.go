package rootapi

// Info holds useful information to display on the ui
type Info struct {
	SshClientURI              string   `json:"SshClientURI"`
	SshClientConnectionStatus string   `json:"SshClientConnectionStatus"`
	JumpHosts                 []string `json:"JumpHosts"`
}

type statsResponse struct {
	CountTunnels        int `json:"CountTunnels"`
	CountTunnelsClients int `json:"CountTunnelsClients"`

	CountPipes        int `json:"CountPipes"`
	CountPipesClients int `json:"CountPipesClients"`

	// runtime stats
	NumGoroutine int    `json:"NumGoroutine"`
	MemTotal     uint64 `json:"MemTotal"`
}
