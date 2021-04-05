# Gotun

Gotun is a very simple ssh tunnel tool.

It's meant to replace the couple autossh - sshd.

Usage example:

Starts an embedded ssh server and proxy the port to raspberrypi.local

```
$ gotun pi@raspberry.local
```

Forwards the local 5000 port to the remote 6000 on the raspberrypi.local host

```
$ gotun -start-ssh=false -local localhost:5000 -remote localhost:6000 pi@raspberry.local
```