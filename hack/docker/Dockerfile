FROM golang:1.16 as gobuilder
WORKDIR /go/src/app
COPY . .
RUN go build -o /rospo .

# Final docker image
FROM ubuntu:latest
COPY --from=gobuilder /rospo /usr/local/bin/rospo