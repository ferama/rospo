package ui

import "embed"

//go:embed build/*
var StaticFiles embed.FS
