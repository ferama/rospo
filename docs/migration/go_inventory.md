# Rospo Go Inventory

This document captures the Go implementation contract that the Rust port must preserve.

## CLI Surface

Root command:

- Binary name: `rospo`
- Root long description: `The tool to create relieable ssh tunnels.`
- Persistent global flag:
  - `-q, --quiet` default `false`
- Root version flag is enabled by Cobra.
- Running `rospo` with no args prints Cobra's arity error and usage and exits with code `0`.
- Root `Run` handler prints `invalid subcommand` and calls `os.Exit(1)`, but in practice Cobra handles most invalid invocations before that path.

Commands:

- `dns-proxy [user@]host[:port]`
  - SSH client flags
  - `-l, --listen-address` default `:53`
  - `-d, --remote-dns-server` default `1.1.1.1:53`
- `get [user@]host[:port] remote [local]`
  - SSH client flags
  - `-w, --max-workers` default `12`
  - `-c, --concurrent-downloads` default `4`
  - `-r, --recursive` default `false`
- `grabpubkey host:port`
  - `-k, --known-hosts` default `$HOME/.ssh/known_hosts`
- `keygen`
  - `-s, --store` default `false`
  - `-p, --path` default `.`
  - `-n, --name` default `identity`
- `put [user@]host[:port] local [remote]`
  - SSH client flags
  - `-w, --max-workers` default `16`
  - `-c, --concurrent-uploads` default `4`
  - `-r, --recursive` default `false`
- `revshell [user@]host[:port]`
  - SSH client flags
  - SSH server flags
  - `-r, --remote` default `127.0.0.1:2222`
- `run config_file_path.yaml`
- `shell [user@]host[:port] [cmd_string]`
  - SSH client flags
- `socks-proxy [user@]host[:port]`
  - SSH client flags
  - `-l, --listen-address` default `127.0.0.1:1080`
- `sshd`
  - SSH server flags
  - `-D, --disable-shell` default `false`
- `template`
- `tun`
  - SSH client persistent flags
  - `-l, --local` default `127.0.0.1:2222`
  - `-r, --remote` default `127.0.0.1:2222`
  - Subcommands:
    - `forward [user@][server]:port`
    - `reverse [user@][server]:port`

Shared SSH client flags:

- `-b, --disable-banner` default `false`
- `-i, --insecure` default `false`
- `-j, --jump-host` default `""`
- `-s, --user-identity` default `$HOME/.ssh/id_rsa`
- `-k, --known-hosts` default `$HOME/.ssh/known_hosts`
- `-p, --password` default `""`

Shared SSH server flags:

- `-K, --sshd-authorized-keys` default `./authorized_keys`
- `-P, --sshd-listen-address` default `:2222`
- `-I, --sshd-key` default `./server_key`
- `-T, --disable-auth` default `false`
- `-A, --sshd-authorized-password` default `""`

## Config Schema

Root YAML object:

- `sshclient`
- `tunnel`
- `sshd`
- `socksproxy`
- `dnsproxy`

`sshclient` schema:

- `identity: string`
- `password: string`
- `known_hosts: string`
- `server: string`
- `insecure: bool`
- `quiet: bool`
- `jump_hosts: []jump_host`

`jump_host` schema:

- `uri: string`
- `identity: string`
- `password: string`

`tunnel` entry schema:

- `remote: string`
- `local: string`
- `forward: bool`
- `sshclient: sshclient`

`socksproxy` schema:

- `listen_address: string`
- `sshclient: sshclient`

`dnsproxy` schema:

- `listen_address: string`
- `remote_dns_address: string | null`
- `sshclient: sshclient`

`sshd` schema:

- `server_key: string`
- `authorized_keys: []string`
- `authorized_password: string`
- `listen_address: string`
- `disable_shell: bool`
- `disable_banner: bool`
- `disable_auth: bool`
- `disable_sftp_subsystem: bool`
- `disable_tunnelling: bool`
- `shell_executable: string`

Observed config parsing behavior:

- Missing sections decode as `nil`.
- Missing booleans decode as `false`.
- Unknown keys are ignored by `yaml.v3`.
- `dnsproxy.remote_dns_address` falls back later at runtime to `1.1.1.1:53` when absent.
- There is no post-load defaulting pass in `pkg/conf`; defaults mostly live in CLI code and runtime constructors.

## Module Map

- `cmd`
  - Cobra command tree and flag wiring
- `pkg/conf`
  - YAML loading into composed runtime config
- `pkg/sshc`
  - SSH client, remote shell, SFTP client, SOCKS proxy, DNS proxy
- `pkg/sshd`
  - Embedded SSH server, PTY/session handling, SFTP subsystem, remote port forwarding handlers
- `pkg/tun`
  - Forward and reverse tunnel engine with reconnect loop and metrics
- `pkg/utils`
  - SSH URL parsing, known_hosts helpers, SSH config parser, key generation, shell lookup
- `pkg/rio`
  - Bidirectional copy helpers with optional byte counters
- `pkg/rpty`
  - PTY abstraction, `creack/pty` on Unix and ConPTY on Windows
- `pkg/logger`
  - Colorized `log.Logger` factory and global disable/enable
- `pkg/worker`
  - Bounded worker pool for SFTP chunk transfers
- `pkg/registry`
  - In-memory object registry used by tunnel registry

## Runtime Behaviors That Must Carry Forward

- SSH client reconnect loop:
  - Retries every 5 seconds.
  - Sends `keepalive@rospo` every 5 seconds while connected.
- Tunnel reconnect loop:
  - Retries every 5 seconds.
  - Forward tunnels listen locally and dial remote over SSH `direct-tcpip`.
  - Reverse tunnels request remote listeners via `tcpip-forward` and accept `forwarded-tcpip`.
- Host key handling:
  - If `insecure` is false and host is unknown, normal connections fail with a fatal log instructing the user to run `grabpubkey`.
  - `grabpubkey` performs a non-failing host key callback and appends the key to `known_hosts`.
  - Missing or unparsable `known_hosts` files are created lazily.
- SSH config integration:
  - `~/.ssh/config` is parsed once via a singleton.
  - Matching host entries can override user, hostname, port, identity file, known-hosts file, strict host key checking, and proxy jump.
- Banner behavior:
  - SSH client prints server banners unless `disable-banner` is set.
  - SSH server prints a frog banner unless disabled or running on Windows.
- SOCKS behavior:
  - Uses `github.com/ferama/go-socks`.
  - SOCKS4 and SOCKS5 are both supported.
- DNS proxy behavior:
  - Listens on both UDP and TCP locally.
  - Sends upstream DNS requests through SSH using DNS-over-TCP framing.
- SFTP behavior:
  - Client supports resumable chunked upload/download with worker pools.
  - Server subsystem can be disabled via config.
- Windows behavior:
  - Binary can run as a Windows service using `go-svc`.
  - PTY is implemented using ConPTY.

## Known Compatibility Traps

- CLI help output ordering and wording come from Cobra and need fixture-driven matching.
- Root no-arg exit code is `0`, despite printing an error.
- `run` has a likely bug in the dedicated DNS proxy SSH client path: it uses `conf.SocksProxy.SshClientConf` instead of `conf.DnsProxy.SshClientConf`.
- `pkg/conf/testdata/sshd.yaml` uses `port`, but the actual struct field is `listen_address`; the test only validates `DisableShell` defaulting, so the mismatch is currently tolerated.
- `worker.NewPool` uses `for range maxWorkers`, which is valid in modern Go and starts exactly `maxWorkers` goroutines.

## Validation Status

- Non-networked Go tests pass in the sandbox.
- Networked Go tests require escalated execution because the sandbox blocks local TCP bind/connect operations.
