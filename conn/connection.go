package conn

import (
	"fmt"
	"gotun/utils"
	"io"
	"log"
	"net"

	"golang.org/x/crypto/ssh"
)

// From https://sosedoff.com/2015/05/25/ssh-port-forwarding-with-go.html
// Handle local client connections and tunnel data to the remote server
// Will use io.Copy - http://golang.org/pkg/io/#Copy
func HandleClient(client net.Conn, remote net.Conn) {
	defer client.Close()
	chDone := make(chan bool)

	// Start remote -> local data transfer
	go func() {
		_, err := io.Copy(client, remote)
		if err != nil {
			log.Println(fmt.Sprintf("error while copy remote->local: %s", err))
		}
		chDone <- true
	}()

	// Start local -> remote data transfer
	go func() {
		_, err := io.Copy(remote, client)
		if err != nil {
			log.Println(fmt.Sprintf("error while copy local->remote: %s", err))
		}
		chDone <- true
	}()

	<-chDone
}

func Connect(serverEndpoint *Endpoint, remoteEndpoint *Endpoint) (*ssh.Client, net.Listener) {
	flags := utils.GetFlags()

	// refer to https://godoc.org/golang.org/x/crypto/ssh for other authentication types
	sshConfig := &ssh.ClientConfig{
		// SSH connection username
		User: *flags.Username,
		Auth: []ssh.AuthMethod{
			utils.PublicKeyFile(*flags.Identity),
			// ssh.Password("your_password_here"),
		},
		HostKeyCallback: ssh.InsecureIgnoreHostKey(),
	}
	// Connect to SSH remote server using serverEndpoint
	serverConn, err := ssh.Dial("tcp", serverEndpoint.String(), sshConfig)
	if err != nil {
		log.Println(fmt.Printf("Dial INTO remote server error. %s\n", err))
		return nil, nil
	}

	// Listen on remote server port
	listener, err := serverConn.Listen("tcp", remoteEndpoint.String())
	if err != nil {
		log.Println(fmt.Printf("Listen open port ON remote server error. %s\n", err))
		return nil, nil
	}

	log.Println("connected")
	return serverConn, listener
}
