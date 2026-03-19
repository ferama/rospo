package main

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/ferama/rospo/pkg/conf"
	"github.com/ferama/rospo/pkg/utils"
)

func fatalf(format string, args ...any) {
	fmt.Fprintf(os.Stderr, format+"\n", args...)
	os.Exit(1)
}

func printJSON(v any) {
	enc := json.NewEncoder(os.Stdout)
	enc.SetIndent("", "  ")
	if err := enc.Encode(v); err != nil {
		fatalf("encode json: %v", err)
	}
}

func main() {
	if len(os.Args) < 3 {
		fatalf("usage: go_baseline <config|ssh-url|ssh-config> <value>")
	}

	switch os.Args[1] {
	case "config":
		cfg, err := conf.LoadConfig(os.Args[2])
		if err != nil {
			fatalf("load config: %v", err)
		}
		printJSON(cfg)
	case "ssh-url":
		printJSON(utils.ParseSSHUrl(os.Args[2]))
	case "ssh-config":
		parser := utils.NewBaselineSSHConfigParser()
		nodes, err := parser.ParseFile(os.Args[2])
		if err != nil {
			fatalf("parse ssh config: %v", err)
		}
		printJSON(nodes)
	default:
		fatalf("unknown mode: %s", os.Args[1])
	}
}
