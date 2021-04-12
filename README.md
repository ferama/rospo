# Rospo

Rospo is a tool meant to create reliable ssh tunnels.
It embeds an ssh server too if you want to reverse proxy a secured
shell

It's meant to make ssh tunnels fun, reliable and understendable again

### Table of Contents  
1. [Why Rospo?](#why-rospo)
2. [How to Install](#how-to-install)
    * [Linux (amd64)](#linux-amd64)
    * [Linux (arm64)](#linux-arm64)
    * [Linux (arm)](#linux-arm)
    * [Mac Os (Apple silicon)](#mac-os)
3. [Usage](#usage)


## Why Rospo
I wanted an easy to use and reliable ssh tunnel tool. The available alternatives doesn't fully satisfy me and doesn't support all the features I need (as the embedded sshd server for example) so I wrote my own

Why use and embedded sshd server you could tell me. 

Example scenario:
You have a Windows WSL instance that you want to access remotely without complicated setups on firewalls and other hassles and annoyances. With **rospo** you can do it in ONE simple step:

```
$ rospo run reverse -S external_ssh_server_here
```

This command will run an embedded sshd server on your wsl instance and reverse proxy its port to the `external_ssh_server_here`

The only assumption here is that you have access to `external_ssh_server_here` using ssh keys.
The command will open a socket (on port 5555 by default) into `external_ssh_server_here` that you can use to log back to WSL using a standard ssh client with a command like:

```
$ ssh -p 5555 localhost
```

But this is just an example. Rospo can do a lot more.

The tunnel is fully secured using standard ssh mechanisms. Rospo will generate server identity file on first run and uses standard `authorized_keys` and user `known_hosts` files.

Rospo tunnel are monitored and keeped up in the event of network issues.

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