package main

import "gotun/sshs"

func main() {
	s := sshs.NewSshServer()
	s.Start()
}
