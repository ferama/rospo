# Rospo

Rospo is a tool meant to create reliable ssh tunnels.
It embeds an ssh server too if you want to reverse proxy a secured
shell

It's meant to make ssh tunnels fun, reliable and understendable again

### Table of Contents  
1. [How to Install](#how-to-install)
    * [Linux (amd64)](#linux-amd64)
    * [Linux (arm64)](#linux-arm64)
    * [Linux (arm)](#linux-arm)
    * [Mac Os (Apple silicon)](#mac-os)
2. [Usage](#usage)


## How to Install

Rospo actually only full supports *nix oses.
A windows version is being evalued

#### Linux amd64
```
curl -L https://github.com/ferama/rospo/releases/latest/download/rospo-linux-amd64 --output rospo && chmod +x rospo
```

#### Linux arm64
```
curl -L https://github.com/ferama/rospo/releases/latest/download/rospo-linux-arm64 --output rospo && chmod +x rospo
```

#### Linux arm
```
curl -L https://github.com/ferama/rospo/releases/latest/download/rospo-linux-arm --output rospo && chmod +x rospo
```

#### Mac OS
```
curl -L https://github.com/ferama/rospo/releases/latest/download/rospo-darwin-arm64 --output rospo && chmod +x rospo
```

## Usage
Usage example:

Starts an embedded ssh server and reverse proxy the port to remote_server

```
$ rospo tun reverse -S -r :8888 user@server:port
```

Forwards the local 5000 port to the remote 6000 on the remote_server

```
$ rospo tun forward -l :5000 -r :6000 user@server:port
```

Get more detailed help on each command runnig
```
$ rospo tun forward --help
$ rospo tun reverse --help
$ rospo sshd --help
```