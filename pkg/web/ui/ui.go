package ui

import "embed"

//go:embed build/*
// StaticFiles includes all ui app static contents
var StaticFiles embed.FS
