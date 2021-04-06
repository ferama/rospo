package tun

import (
	"fmt"
	"gotun/utils"
	"log"
	"net"

	"golang.org/x/crypto/ssh"
)

func ForwardTunnel(
	username string,
	identity string,
	serverEndpoint *Endpoint,
	remoteEndpoint *Endpoint,
	localEndpoint *Endpoint,
) {

	// refer to https://godoc.org/golang.org/x/crypto/ssh for other authentication types
	sshConfig := &ssh.ClientConfig{
		// SSH connection username
		User: username,
		Auth: []ssh.AuthMethod{
			utils.PublicKeyFile(identity),
			// ssh.Password("your_password_here"),
		},
		HostKeyCallback: ssh.InsecureIgnoreHostKey(),
	}
	// Connect to SSH remote server using serverEndpoint
	serverConn, err := ssh.Dial("tcp", serverEndpoint.String(), sshConfig)
	if err != nil {
		log.Println(fmt.Printf("[TUN] Dial INTO remote server error. %s\n", err))
		return
	}

	// Listen on remote server port
	listener, err := net.Listen("tcp", localEndpoint.String())
	if err != nil {
		log.Println(fmt.Printf("[TUN] Dial INTO remote service error. %s\n", err))
		return
	}

	log.Printf("[TUN] Forward connected. Local: %s <- Remote: %s\n", localEndpoint.String(), remoteEndpoint.String())
	if serverConn != nil && listener != nil {
		for {
			remote, err := serverConn.Dial("tcp", remoteEndpoint.String())
			// Open a (local) connection to localEndpoint whose content will be forwarded so serverEndpoint
			if err != nil {
				log.Println(fmt.Printf("[TUN] Listan open port ON local server error. %s\n", err))
				break
			}

			client, err := listener.Accept()
			if err != nil {
				log.Println("[TUN] disconnected")
				break
			}
			serveClient(client, remote)
		}
		serverConn.Close()
		listener.Close()
	}
}
