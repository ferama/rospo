//go:build !windows

package main

import "github.com/ferama/rospo/cmd"

func main() {
	cmd.Execute()
}
