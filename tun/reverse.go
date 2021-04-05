package tun

import (
	"fmt"
	"gotun/utils"
	"log"
	"net"

	"golang.org/x/crypto/ssh"
)

func ReverseTunnel(
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
	listener, err := serverConn.Listen("tcp", remoteEndpoint.String())
	if err != nil {
		log.Println(fmt.Printf("[TUN] Listen open port ON remote server error. %s\n", err))
		return
	}

	log.Println("[TUN] connected")
	if serverConn != nil && listener != nil {
		for {
			// Open a (local) connection to localEndpoint whose content will be forwarded so serverEndpoint
			local, err := net.Dial("tcp", localEndpoint.String())
			if err != nil {
				log.Println(fmt.Printf("[TUN] Dial INTO local service error. %s\n", err))
				break
			}

			client, err := listener.Accept()
			if err != nil {
				log.Println("[TUN] disconnected")
				break
			}
			serveClient(client, local)
		}
		serverConn.Close()
		listener.Close()
	}
}
