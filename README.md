# Gotun

Gotun is a very simple ssh tunnel tool.

It's meant to replace the couple autossh - sshd.

Usage example:

```
# forward the local 5000 port to the remote 6000 on the raspberrypi.local host

$ gotun -local localhost:5000 -remote localhost:6000 pi@raspberry.local
```