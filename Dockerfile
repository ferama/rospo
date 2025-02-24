# go backend builder
FROM golang:1.24 as gobuilder
ARG VERSION=development
WORKDIR /go/src/app
COPY . .
RUN go build \
    -trimpath \
    -ldflags="-s -w -X 'github.com/ferama/rospo/cmd.Version=$VERSION'" \
    -o /rospo .

# Final docker image
FROM debian:stable-slim
RUN set -eux; \
    apt update && \
    apt install -y \
        ca-certificates \
        curl \
        psmisc \
        procps \
        iputils-ping \
        netcat-openbsd \
        dnsutils \
    && \
    apt clean

COPY --from=gobuilder /rospo /usr/local/bin/rospo

ENTRYPOINT ["/usr/local/bin/rospo"]