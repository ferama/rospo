#! /bin/bash

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd $DIR

cd $DIR/pkg/web/ui && npm install && npm run build && cd $DIR

goreleaser release  --rm-dist
