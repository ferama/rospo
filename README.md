# [Rospo](https://github.com/ferama/rospo)
[![Go Reference](https://pkg.go.dev/badge/github.com/ferama/rospo.svg)](https://pkg.go.dev/github.com/ferama/rospo)
[![Go Report Card](https://goreportcard.com/badge/github.com/ferama/rospo)](https://goreportcard.com/report/github.com/ferama/rospo)
[![codecov](https://codecov.io/gh/ferama/rospo/branch/main/graph/badge.svg)](https://codecov.io/gh/ferama/rospo)
[![Docker Pulls](https://img.shields.io/docker/pulls/ferama/rospo.svg)](https://hub.docker.com/r/ferama/rospo/)




Rospo is a tool meant to create reliable ssh tunnels.
It embeds an ssh server too if you want to reverse proxy a secured
shell

It's meant to make ssh tunnels fun and understendable again

### Table of Contents  
1. [Why Rospo?](#why-rospo)
2. [Quick command line usage](#quick-command-line-usage)
3. [Rospo UI](#rospo-ui)
4. [Scenarios](#scenarios)
    * [Example scenario: Windows (WSL || PowerShell) reverse shell](#example-scenario-windows-reverse-shell)
    * [Example scenario: multiple complex tunnels](#example-scenario-multiple-complex-tunnels)
    * [Example scenario: kubernetes service exporter](#example-scenario-kubernetes-service-exporter)
5. [How to Install](#how-to-install)


## Why Rospo
I wanted an easy to use and reliable ssh tunnel tool. The available alternatives don't fully satisfy me and don't support all the features I need (as the embedded sshd server for example, or an out of the box connection monitoring mechanism) so I wrote my own

## Quick command line usage
Rospo supports keys based auth and password auth. Keys based one is always the preferred, so it is better if *identity*, *authorized_keys* etc are always correctly setup.

Usage example:

Starts an embedded ssh server and reverse proxy the port (2222 by default) to remote_server

```
$ rospo revshell user@server:port
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

For more complex use cases and more options, you can use a config file
```
$ rospo config.yaml
```

Look at the [config_template.yaml](https://github.com/ferama/rospo/blob/main/configs/config_template.yaml) for all the available options.

A config file is required for example to setup pipes. Pipes let's you do things like:

1. opening a socket on locahost on port 1234
2. copy all packets from and to local port 1234 to remote reachable host:whathever_port

This is handy in some situations when you want to use a host as bridge for a service (its almost like a **socat** bidirectional pipe but without the need for another tool)

## Rospo UI
Rospo supports a cool ui too. The ui will let you handle tunnels and pipes configuration at runtime through the web interface.
You can start/stop new tunnels and pipes at runtime.

Pipes and tunnels that are configured through the rospo config file will not be administrable from the ui.

![Image of Home](https://raw.githubusercontent.com/ferama/rospo/main/img/home.png)

![Image of tunnels](https://raw.githubusercontent.com/ferama/rospo/main/img/tunnels.png)

![Image of tunnels](https://raw.githubusercontent.com/ferama/rospo/main/img/pipes.png)

## Scenarios

### Example scenario: Windows reverse shell
Why use an embedded sshd server you might ask me. 
Suppose you have a Windows WSL instance that you want to access remotely without complicated setups on firewalls and other hassles and annoyances. With **rospo** you can do it in ONE simple step:

```
$ rospo revshell remote_ssh_server
```

This command will run an embedded sshd server on your wsl instance and reverse proxy its port to the `remote_ssh_server`

The only assumption here is that you have access to `remote_ssh_server`.
The command will open a socket (on port 2222 by default) into `remote_ssh_server` that you can use to log back to WSL using a standard ssh client with a command like:

```
$ ssh -p 2222 localhost
```

Or even better (why not!) with rospo you can reverse proxy a powershell.
Using rospo for windows:
```
rospo.exe revshell remote_ssh_server
```



### Example scenario: multiple complex tunnels

Rospo supports multiple tunnels on the same ssh connetion. To exploit the full power of rospo for more complex cases, you should/need to use a scenario config file.
Let's define one. Create a file named `config.yaml` with the following contents
```yaml
sshclient:
  server: myuser@remote_server_address
  identity: "~/.ssh/id_rsa"
  jump_hosts:
    - uri: anotheruser@jumphost_address
      identity: "~/.ssh/id_rsa"

tunnel:
  - remote: ":8000"
    local: ":8000"
    forward: yes
  - remote: ":9999"
    local: ":9999"
    forward: yes
  - remote: ":5000"
    local: ":5000"
    forward: no
```

Launch rospo using the config file instead of the cli parameters:
```
$ rospo config.yaml
```

What's happens here is that rospo will connect to `remote_server_address` through the `jumphost_address` server and will:

1. open a socket on the local machine listening on port 8000 that forwards all the traffic to the service listening on port 8000 on the `remote_server_address` machine
2. open a socket on the local machine listening on port 9999 that forwards all the traffic to the service listening on port 9999 on the `remote_server_address` machine
3. open a socket on the remote machine listening on port 5000 that forwards all the traffic from remote machine to a local service (on the local machine) listening on port 5000

But these are just an examples. Rospo can do a lot more.

Tunnels are fully secured using standard ssh mechanisms. Rospo will generate server identity file on first run and uses standard `authorized_keys` and user `known_hosts` files.

Rospo tunnel are monitored and keeped up in the event of network issues.


### Example scenario: kubernetes service exporter

Many times during development on k8s you need to port-forward some of the pods services for local development and/or tests. You need the port forward maybe because that services are not meant to be exposed through the internet or for whatever reason.

Rospo can come to the rescue here. You can create a `rospo.conf` like this:
```yaml
sshclient:
  identity: "/etc/rospo/id_rsa"
  server: my-rospo-or-standard-sshd-server:2222
  known_hosts: "/etc/rospo/known_hosts"

tunnel:
  - remote: "0.0.0.0:9200"
    local: ":9200"
    forward: no
  - remote: "0.0.0.0:8080"
    local: ":8080"
    forward: no

pipe:
  - remote: "elasticsearch-master.mynamespace:9200"
    local: ":9200"
  - remote: "demo-app.mynamespace:8080"
    local: ":8080"
```

You need to create the keys accordingly and put them correctly on the target server. After that you can run a kubernetes pod that keeps up the tunnels and let you securely access the services from a machine inside your local network.
Please refer to the example in [./hack/k8s](./hack/k8s) for more details.

In this scenario the k8s pods act as a bridge between kubernetes services and the reverse tunnels. You are going to use `pipes` to copy the connections from the services to the rospo pod. The pipes in the example will open 2 sockets locally inside the pod:
  1. a socket on local port **9200** for the **elasticsearch-master.mynamespace:9200** service
  2. a socket on local port **8080** for the **demo-app.mynamespace:8080** service

 Finally you are going to reverse forward the pod local ports to the desired host (my-rospo-or-standard-sshd-server:2222 in the example above)

## How to Install

Rospo actually full supports *nix oses and Windows 10
Grab the latest binary release from here [https://github.com/ferama/rospo/releases/latest](https://github.com/ferama/rospo/releases/latest) or use the copy and paste curl below

Alternatively you can use the docker ditribution where useful/needed. Look at an example on kubernetes here [./hack/k8s](./hack/k8s) 


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

#### Windows

You will require Windows 10

```
(New-Object System.Net.WebClient).DownloadFile("https://github.com/ferama/rospo/releases/latest/download/rospo-windows-amd64.exe", "rospo.exe")
```


