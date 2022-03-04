# [Rospo](https://github.com/ferama/rospo)
[![Go Reference](https://pkg.go.dev/badge/github.com/ferama/rospo.svg)](https://pkg.go.dev/github.com/ferama/rospo)
[![Go Report Card](https://goreportcard.com/badge/github.com/ferama/rospo)](https://goreportcard.com/report/github.com/ferama/rospo)
[![codecov](https://codecov.io/gh/ferama/rospo/branch/main/graph/badge.svg)](https://codecov.io/gh/ferama/rospo)


Rospo is a tool meant to create secure and reliable SSH tunnels. A single binary includes both client and server.
It's meant to make SSH tunnels fun and understendable again

### Table of Contents  
1. [Features](#features)
2. [How to Install](#how-to-install)
3. [Quick command line usage](#quick-command-line-usage)
4. [Rospo UI](#rospo-ui)
5. [Example Scenarios](#scenarios)
    * [Windows (WSL || PowerShell) reverse shell](#example-scenario-windows-reverse-shell)
    * [Windows service to reverse tunnel Remote Desktop](#example-scenario-windows-service)
    * [Multiple complex tunnels](#example-scenario-multiple-complex-tunnels)
    * [Kubernetes service exporter](#example-scenario-kubernetes-service-exporter)


## Features

  * Easy to use (single binary client/server functionalities)
  * Encrypted connections through ssh ( `crypto/ssh` package )
  * Automatic connection monitoring to keep it always up
  * Embedded sshd server
  * Forward and reverse tunnels support
  * JumpHosts support
  * Command line options or `human readable` yaml config file
  * Run as a Windows Service support
  * Pty on Windows through conpty apis

## How to Install

Rospo actually full supports *nix oses and Windows 10+

### macOS
#### Homebrew
Install rospo using [Homebrew](http://brew.sh/)

```
brew install rospo
```

### GNU/Linux
#### Binary Download
| Platform | Architecture | URL |
| ---------- | -------- |------|
|GNU/Linux|amd64|https://github.com/ferama/rospo/releases/latest/download/rospo-linux-amd64 |
||arm64|https://github.com/ferama/rospo/releases/latest/download/rospo-linux-arm64|
||arm|https://github.com/ferama/rospo/releases/latest/download/rospo-linux-arm|


### Microsoft Windows
#### Binary Download
| Platform | Architecture | URL |
| ---------- | -------- |------|
|Microsoft Windows|amd64|https://github.com/ferama/rospo/releases/latest/download/rospo-windows-amd64.exe|


### Docker Container
You can use the docker ditribution where useful/needed. Look at an example on kubernetes here [./hack/k8s](./hack/k8s) 
```
docker pull ghcr.io/ferama/rospo
docker run ghcr.io/ferama/rospo rospo help
```

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

## Rospo UI
Rospo supports a cool ui too. The ui will let you handle tunnels configuration at runtime through the web interface.
You can start/stop new tunnels at runtime.

Tunnels that are configured through the rospo config file will not be administrable from the ui.

![Image of Home](https://raw.githubusercontent.com/ferama/rospo/main/img/home.png)

![Image of tunnels](https://raw.githubusercontent.com/ferama/rospo/main/img/tunnels.png)

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

### Example scenario: Windows service
Rospo support execution as a service on windows. This means that you can create
a persistent tunnel that can be installed as a service and started automatically with
the machine.

Let's do this with the Windows Remote Desktop service.

Create a rospo conf file like this:
```yaml
sshclient:
  server: your-rospo-or-sshd-server-uri:2222
  identity: "c:\\absolute_path_to_your\\id_rsa"
  known_hosts: "C:\\absolute_path_to_your\\known_hosts"

tunnel:
  - remote: :3389
    local: :3389  # the windows remote desktop port
    forward: false
```

Launch a terminal (powershell) with Administrative rights.
You can then perform the following actions:

```powershell
# create the rospo service
sc.exe create rospo start= auto type= own DisplayName= Rospo binpath= "C:\rospo.exe C:\rospo_conf.yaml"

# start service
sc.exe start rospo

# query service status
sc.exe query rospo

# stop and delete the service
sc.exe stop rospo; sc.exe delete rospo
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
    local: "elasticsearch-master.mynamespace:9200"
    forward: no
  - remote: "0.0.0.0:8080"
    local: "demo-app.mynamespace:8080"
    forward: no

```

You need to create the keys accordingly and put them correctly on the target server. After that you can run a kubernetes pod that keeps up the tunnels and let you securely access the services from a machine inside your local network.
Please refer to the example in [./hack/k8s](./hack/k8s) for more details.

In this scenario the k8s pods act as a bridge between kubernetes services and the reverse tunnels. 

You are going to reverse forward the pod local reachable services ports to the desired host (my-rospo-or-standard-sshd-server:2222 in the example above)




