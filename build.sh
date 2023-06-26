#! /bin/bash

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd $DIR

if ! command -v git &> /dev/null
then
    DEV_VER="development"
else
    DEV_VER="dev-$(git rev-parse --short HEAD)"
fi

VERSION=${VERSION:=$DEV_VER}

build() {
    EXT=""
    [[ $GOOS = "windows" ]] && EXT=".exe"
    echo "Building ${GOOS} ${GOARCH}"
    CGO_ENABLED=0 go build \
        -trimpath \
        -ldflags="-s -w -X 'github.com/ferama/rospo/cmd.Version=$VERSION'" \
        -o ./bin/rospo-${GOOS}-${GOARCH}${EXT} .
}

### test units
go clean -testcache
go test ./... -v -cover -race || exit 1


### multi arch binary build
GOOS=linux GOARCH=arm build
GOOS=linux GOARCH=arm64 build
GOOS=linux GOARCH=amd64 build

GOOS=darwin GOARCH=arm64 build
GOOS=darwin GOARCH=amd64 build

GOOS=windows GOARCH=amd64 build

GOOS=linux GOMIPS=softfloat GOARCH=mips build
GOOS=linux GOMIPS=softfloat GOARCH=mipsle build
