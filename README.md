# Rospo

Rospo is a very simple ssh tunnel tool.

It's meant to replace the couple autossh - sshd for forwards and reverse tunnels.

Usage example:

Starts an embedded ssh server and proxy the port to remote_server

```
$ rospo user@remote_server:port
```

Forwards the local 5000 port to the remote 6000 on the remote_server

```
$ rospo -no-sshd -local localhost:5000 -remote localhost:6000 user@remote_server:port
```