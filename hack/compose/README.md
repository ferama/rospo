# Start compose

This is an example on how to run rospo as an sshd on docker

Generate keys with

```sh
rospo keygen -n server_key -s
```

Create authorized_keys

```sh
touch authorized_keys
```

Run with

```sh
docker compose up -d
```