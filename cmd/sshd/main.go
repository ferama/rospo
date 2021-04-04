package main

import "gotun/sshd"

func main() {
	s := sshd.NewSshServer()
	s.Start()
}
