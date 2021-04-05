package main

import (
	"gotun/sshd"
)

func main() {
	identity := "./id_rsa"
	auth_keys := "./authorized_keys"
	port := "2222"
	s := sshd.NewSshServer(
		&identity,
		&auth_keys,
		&port,
	)
	s.Start()
}
