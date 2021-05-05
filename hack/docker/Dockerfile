FROM golang:1.16 as gobuilder
WORKDIR /go/src/app
COPY . .
RUN go build -o /rospo .

# Final docker image
FROM ubuntu:20.04
COPY --from=gobuilder /rospo /usr/local/bin/rospo