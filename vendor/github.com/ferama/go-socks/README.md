# go-socks

Provides the `socks` package that implements a [SOCKS server](http://en.wikipedia.org/wiki/SOCKS).
SOCKS (Secure Sockets) is used to route traffic between a client and server through
an intermediate proxy layer. This can be used to bypass firewalls or NATs.

## Feature

The package has the following features:
* "No Auth" mode
* User/Password authentication
* Support for the CONNECT command
* Rules to do granular filtering of commands
* Custom DNS resolution
* Unit tests

## TODO

The package still needs the following:
* Support for the BIND command
* Support for the ASSOCIATE command


## Example

Below is a simple example of usage

```go
// Create a SOCKS server
conf := &socks.Config{}
server, err := socks.New(conf)
if err != nil {
  panic(err)
}

// Create SOCKS proxy on localhost port 1080
if err := server.ListenAndServe("tcp", "127.0.0.1:1080"); err != nil {
  panic(err)
}
```

