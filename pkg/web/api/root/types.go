package rootapi

// Info holds useful information to display on the ui
type Info struct {
	SshClientURI              string   `json:"SshClientURI"`
	SshClientConnectionStatus string   `json:"SshClientConnectionStatus"`
	JumpHosts                 []string `json:"JumpHosts"`
}
