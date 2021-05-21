#! /bin/bash

VERSION=${VERSION:=development}

build() {
    EXT=""
    [[ $GOOS = "windows" ]] && EXT=".exe"
    echo "Building ${GOOS} ${GOARCH}"
    go build \
        -ldflags="-X 'github.com/ferama/rospo/cmd.Version=$VERSION'" \
        -o ./bin/rospo-${GOOS}-${GOARCH}${EXT} .
}

go test ./... -v

GOOS=linux GOARCH=arm build
GOOS=linux GOARCH=arm64 build
GOOS=linux GOARCH=amd64 build

GOOS=darwin GOARCH=arm64 build

GOOS=windows GOARCH=amd64 build