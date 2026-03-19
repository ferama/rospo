# Rospo Migration Spec

Date: 2026-03-19

This document is the exhaustive specification assembled so far for the Go-to-Rust migration of `rospo`. It combines:

- the Go compatibility contract already extracted from the original codebase
- the golden baselines captured from the Go binary and Go packages
- the current Rust implementation status as it exists in this repository

This is not a claim that the Rust port is complete. It is the current source of truth for what must match and what has already been ported.

## Scope

The end goal remains a drop-in Rust replacement for the Go binary with:

- identical command names
- identical flags and defaults
- identical help output
- identical config schema and config-loading behavior
- identical runtime behavior for SSH, tunnels, SOCKS, DNS proxying, SFTP, logging, and exit codes
- interoperability with the Go implementation in both directions

## Source Of Truth

Current compatibility oracles in the repository:

- Go contract summary: `docs/migration/go_inventory.md`
- Interim migration report: `docs/migration/report.md`
- Go CLI golden fixtures: `compat/golden/cli`
- Go runtime/config/parser fixtures: `compat/golden/runtime`
- Go baseline capture script: `scripts/capture_go_baselines.sh`
- Go baseline helper: `tools/go_baseline/main.go`
- Go SSH config parser adapter: `pkg/utils/ssh_config_parser_baseline.go`
- Rust compatibility tests:
  - `rust/tests/cli_compat.rs`
  - `rust/tests/config_compat.rs`
  - `rust/tests/utils_compat.rs`

## CLI Contract

### Root Command

- Binary name: `rospo`
- Root long description: `The tool to create relieable ssh tunnels.`
- Persistent global flag:
  - `-q, --quiet`
  - default: `false`
- Cobra version flag is enabled in Go.
- Observed root behaviors in Go:
  - `rospo --help` prints help and exits `0`
  - `rospo -h` prints help and exits `0`
  - `rospo --version` prints version and exits `0`
  - `rospo` with no args prints Cobra arity error plus usage and exits `0`
  - invalid subcommands generally get handled by Cobra before the root fallback path
- Captured root fixtures:
  - `compat/golden/cli/root-help.txt`
  - `compat/golden/cli/root-help.exitcode`
  - `compat/golden/cli/root-noargs.txt`
  - `compat/golden/cli/root-noargs.exitcode`

### Shared SSH Client Flags

These flags are part of the Go CLI contract and apply where the Go code wires SSH client behavior:

- `-b, --disable-banner`
  - default: `false`
- `-i, --insecure`
  - default: `false`
- `-j, --jump-host`
  - default: `""`
- `-s, --user-identity`
  - default: `$HOME/.ssh/id_rsa`
- `-k, --known-hosts`
  - default: `$HOME/.ssh/known_hosts`
- `-p, --password`
  - default: `""`

### Shared SSH Server Flags

- `-K, --sshd-authorized-keys`
  - default: `./authorized_keys`
- `-P, --sshd-listen-address`
  - default: `:2222`
- `-I, --sshd-key`
  - default: `./server_key`
- `-T, --disable-auth`
  - default: `false`
- `-A, --sshd-authorized-password`
  - default: `""`

### Commands

#### `dns-proxy [user@]host[:port]`

Flags:

- shared SSH client flags
- `-l, --listen-address`
  - default: `:53`
- `-d, --remote-dns-server`
  - default: `1.1.1.1:53`

Purpose in Go:

- runs a local DNS proxy
- listens on both UDP and TCP locally
- forwards DNS requests through SSH
- uses DNS-over-TCP framing upstream

Golden help fixtures:

- `compat/golden/cli/dns-proxy-help.txt`
- `compat/golden/cli/dns-proxy-help.exitcode`

Current Rust status:

- help output is matched from the Go fixture
- runtime implementation is not complete

#### `get [user@]host[:port] remote [local]`

Flags:

- shared SSH client flags
- `-w, --max-workers`
  - default: `12`
- `-c, --concurrent-downloads`
  - default: `4`
- `-r, --recursive`
  - default: `false`

Purpose in Go:

- downloads files via SFTP
- supports concurrent chunked downloads
- supports recursive transfer

Golden help fixtures:

- `compat/golden/cli/get-help.txt`
- `compat/golden/cli/get-help.exitcode`

Current Rust status:

- help output is matched from the Go fixture
- runtime implementation is not complete

#### `grabpubkey host:port`

Flags:

- `-k, --known-hosts`
  - default: `$HOME/.ssh/known_hosts`

Purpose in Go:

- connects to the SSH server
- accepts the host key without failing verification
- appends the host key to the known-hosts file

Golden help fixtures:

- `compat/golden/cli/grabpubkey-help.txt`
- `compat/golden/cli/grabpubkey-help.exitcode`

Current Rust status:

- help output is matched from the Go fixture
- command is implemented
- server public key is fetched using a real SSH handshake via `russh`
- known-hosts entries are appended in the Go-compatible format
- default known-hosts path is expanded from `~/.ssh/known_hosts`

#### `keygen`

Flags:

- `-s, --store`
  - default: `false`
- `-p, --path`
  - default: `.`
- `-n, --name`
  - default: `identity`

Purpose in Go:

- generates an ECDSA P-521 keypair
- either prints the private and public key to stdout or stores them to disk

Golden help fixtures:

- `compat/golden/cli/keygen-help.txt`
- `compat/golden/cli/keygen-help.exitcode`

Current Rust status:

- help output is matched from the Go fixture
- command is implemented
- private key format is SEC1 PEM with `BEGIN EC PRIVATE KEY`
- public key format is OpenSSH `ecdsa-sha2-nistp521`
- stored files are written with mode `0600` on Unix

#### `put [user@]host[:port] local [remote]`

Flags:

- shared SSH client flags
- `-w, --max-workers`
  - default: `16`
- `-c, --concurrent-uploads`
  - default: `4`
- `-r, --recursive`
  - default: `false`

Purpose in Go:

- uploads files via SFTP
- supports concurrent chunked uploads
- supports recursive transfer

Golden help fixtures:

- `compat/golden/cli/put-help.txt`
- `compat/golden/cli/put-help.exitcode`

Current Rust status:

- help output is matched from the Go fixture
- runtime implementation is not complete

#### `revshell [user@]host[:port]`

Flags:

- shared SSH client flags
- shared SSH server flags
- `-r, --remote`
  - default: `127.0.0.1:2222`

Purpose in Go:

- sets up a reverse shell path combining client and embedded server capabilities

Golden help fixtures:

- `compat/golden/cli/revshell-help.txt`
- `compat/golden/cli/revshell-help.exitcode`

Current Rust status:

- help output is matched from the Go fixture
- runtime implementation is not complete

#### `run config_file_path.yaml`

Flags:

- no dedicated flags beyond root/global handling in captured help

Purpose in Go:

- loads YAML config
- starts configured components:
  - SSH client
  - tunnels
  - embedded SSH server
  - SOCKS proxy
  - DNS proxy

Golden help fixtures:

- `compat/golden/cli/run-help.txt`
- `compat/golden/cli/run-help.exitcode`

Current Rust status:

- help output is matched from the Go fixture
- YAML config file is loaded and parsed
- if all top-level sections are absent or empty, Rust returns `2026/03/19 00:00:00 nothing to run\n`
- any non-empty config currently returns `Rust runtime implementation is not complete yet\n` with exit code `1`

#### `shell [user@]host[:port] [cmd_string]`

Flags:

- shared SSH client flags

Purpose in Go:

- connects over SSH
- either runs an interactive shell or executes a command
- validates host keys unless insecure mode disables that
- authenticates using key and/or password
- prints server banners unless disabled

Golden help fixtures:

- `compat/golden/cli/shell-help.txt`
- `compat/golden/cli/shell-help.exitcode`

Current Rust status:

- help output is matched from the Go fixture
- command is implemented for a real SSH client path
- supported in Rust today:
  - host key verification against known-hosts unless insecure mode is enabled
  - key authentication when a readable identity file is available
  - password authentication when `--password` is provided
  - `authenticate_none` fallback
  - banner printing unless `--disable-banner` is set
  - remote command execution
  - interactive shell request
  - basic PTY request
  - environment variable forwarding for:
    - `LANG`
    - `LANGUAGE`
    - `LC_CTYPE`
    - `LC_NUMERIC`
    - `LC_TIME`
    - `LC_COLLATE`
    - `LC_MONETARY`
    - `LC_MESSAGES`
    - `LC_PAPER`
    - `LC_NAME`
    - `LC_ADDRESS`
    - `LC_TELEPHONE`
    - `LC_MEASUREMENT`
    - `LC_IDENTIFICATION`
    - `LC_ALL`
- current Rust gaps for `shell`:
  - `--jump-host` is parsed but ignored
  - no reconnect or keepalive behavior
  - no PTY resize handling
  - no password prompt workflow beyond direct flag use
  - no evidence yet that stderr/stdout/exit-code edge cases match Go in all cases
  - one observed no-auth server path failed during live testing

#### `socks-proxy [user@]host[:port]`

Flags:

- shared SSH client flags
- `-l, --listen-address`
  - default: `127.0.0.1:1080`

Purpose in Go:

- runs a local SOCKS proxy over SSH
- supports SOCKS4 and SOCKS5

Golden help fixtures:

- `compat/golden/cli/socks-proxy-help.txt`
- `compat/golden/cli/socks-proxy-help.exitcode`

Current Rust status:

- help output is matched from the Go fixture
- runtime implementation is not complete

#### `sshd`

Flags:

- shared SSH server flags
- `-D, --disable-shell`
  - default: `false`

Purpose in Go:

- runs the embedded SSH server
- supports auth by key and password
- can disable auth
- can disable shell
- supports SFTP subsystem unless disabled
- supports tunnelling unless disabled

Golden help fixtures:

- `compat/golden/cli/sshd-help.txt`
- `compat/golden/cli/sshd-help.exitcode`

Current Rust status:

- help output is matched from the Go fixture
- runtime implementation is not complete

#### `template`

Flags:

- no command-specific flags

Purpose in Go:

- prints the bundled config template

Golden fixtures:

- `compat/golden/cli/template-help.txt`
- `compat/golden/cli/template-help.exitcode`
- `compat/golden/cli/template-output.txt`
- `compat/golden/cli/template-output.exitcode`

Current Rust status:

- help output is matched from the Go fixture
- runtime behavior is implemented
- Rust reads `cmd/configs/config_template.yaml` directly and appends a trailing newline

#### `tun`

Persistent flags:

- shared SSH client flags
- `-l, --local`
  - default: `127.0.0.1:2222`
- `-r, --remote`
  - default: `127.0.0.1:2222`

Subcommands:

- `tun forward [user@][server]:port`
- `tun reverse [user@][server]:port`

Purpose in Go:

- manages forward and reverse tunnels over SSH
- reconnects persistently

Golden help fixtures:

- `compat/golden/cli/tun-help.txt`
- `compat/golden/cli/tun-help.exitcode`
- `compat/golden/cli/tun-forward-help.txt`
- `compat/golden/cli/tun-forward-help.exitcode`
- `compat/golden/cli/tun-reverse-help.txt`
- `compat/golden/cli/tun-reverse-help.exitcode`

Current Rust status:

- help output is matched from the Go fixture
- runtime implementation is not complete

## Config Schema Contract

The root YAML object may contain:

- `sshclient`
- `tunnel`
- `sshd`
- `socksproxy`
- `dnsproxy`

### `sshclient`

Fields:

- `identity: string`
- `password: string`
- `known_hosts: string`
- `server: string`
- `insecure: bool`
- `quiet: bool`
- `jump_hosts: []jump_host`

### `jump_host`

Fields:

- `uri: string`
- `identity: string`
- `password: string`

### `tunnel`

This is a sequence of tunnel entries.

Each `tunnel` entry contains:

- `remote: string`
- `local: string`
- `forward: bool`
- `sshclient: sshclient`

### `socksproxy`

Fields:

- `listen_address: string`
- `sshclient: sshclient`

### `dnsproxy`

Fields:

- `listen_address: string`
- `remote_dns_address: string | null`
- `sshclient: sshclient`

### `sshd`

Fields:

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

## Config Parsing Rules

Observed from the Go implementation and preserved in current Rust config types:

- missing top-level sections decode as `nil` in Go and `None` in Rust
- missing booleans decode as `false`
- missing strings decode as empty string when the field exists in a present struct
- missing sequences decode as empty vectors when the containing struct is present
- unknown YAML keys are ignored by Go `yaml.v3`
- `dnsproxy.remote_dns_address` is not defaulted during YAML load; runtime later falls back to `1.1.1.1:53`
- there is no central post-load defaulting pass in Go `pkg/conf`
- defaults mainly live in CLI flag definitions and runtime constructors

Known fixture caveat:

- `pkg/conf/testdata/sshd.yaml` uses `port`, but the actual schema field is `listen_address`
- the Go test using that file only validates `DisableShell` defaulting, so the mismatch is currently tolerated

Current Rust config implementation status:

- `rust/src/config/mod.rs` mirrors the YAML field names with `serde(rename = "...")`
- `load_config` is a direct `serde_yaml::from_str`
- compatibility tests currently verify:
  - `pkg/conf/testdata/sshc.yaml`
  - `pkg/conf/testdata/sshc_insecure.yaml`
  - `pkg/conf/testdata/sshc_secure_default.yaml`

## Runtime Behavior Rules To Preserve

### SSH Client

Go behavior that the Rust port must preserve:

- reconnect loop retries every 5 seconds
- keepalive request `keepalive@rospo` is sent every 5 seconds while connected
- host key verification fails normal connections when the host is unknown and `insecure` is false
- the failure message instructs the user to run `grabpubkey`
- `grabpubkey` enrolls host keys without strict verification failure
- missing or unparsable known-hosts files are created lazily
- `~/.ssh/config` is parsed once through a singleton parser
- SSH config host entries may override:
  - user
  - hostname
  - port
  - identity file
  - known-hosts file
  - strict host key checking
  - proxy jump
- client prints server banners unless `disable-banner` is set

Current Rust implementation status:

- real SSH client transport exists in `rust/src/ssh/mod.rs`
- inactivity timeout is set to 5 seconds in `russh` client config
- host key verification against known-hosts is implemented
- banner display suppression is implemented
- no reconnect loop yet
- no Go-compatible keepalive loop yet
- no SSH config integration yet
- no jump-host path yet

### SSH Server

Go behavior that the Rust port must preserve:

- embedded server supports shell/session handling
- embedded server supports key auth and password auth
- auth can be disabled
- SFTP subsystem can be disabled
- remote tunnelling can be disabled
- frog banner is printed unless disabled or on Windows

Current Rust status:

- no embedded SSH server runtime exists yet

### Tunnels

Go behavior that the Rust port must preserve:

- reconnect every 5 seconds
- forward tunnel:
  - local listener
  - remote dial via SSH `direct-tcpip`
- reverse tunnel:
  - remote listener via `tcpip-forward`
  - accepted channels via `forwarded-tcpip`
- server-side liveness handling uses `checkalive@rospo`

Current Rust status:

- only the reconnection interval constant exists in `rust/src/tunnel/mod.rs`
- no tunnel runtime implementation exists yet

### SOCKS Proxy

Go behavior that the Rust port must preserve:

- uses `github.com/ferama/go-socks`
- supports SOCKS4 and SOCKS5

Current Rust status:

- only `DEFAULT_LISTEN_ADDRESS` constant exists in `rust/src/socks/mod.rs`
- no SOCKS runtime implementation exists yet

### DNS Proxy

Go behavior that the Rust port must preserve:

- local UDP listener
- local TCP listener
- upstream DNS over SSH using TCP framing
- default remote DNS server `1.1.1.1:53`

Current Rust status:

- no runtime implementation exists yet

### SFTP

Go behavior that the Rust port must preserve:

- resumable chunked upload and download
- bounded worker pools
- recursive transfer support
- embedded server subsystem support

Current Rust status:

- only `DEFAULT_CHUNK_SIZE` constant exists in `rust/src/sftp/mod.rs`
- no client or server runtime implementation exists yet

### Logging

Go behavior that the Rust port must preserve:

- colorized `log.Logger` output
- global quiet/disable handling
- command/runtime log prefixes and formatting

Current Rust status:

- `rust/src/logging/mod.rs` only contains a placeholder initializer
- output formatting is not matched yet

### Windows / Cross-Platform

Go behavior that the Rust port must preserve:

- Linux support
- macOS support
- Windows support
- Windows service mode via `go-svc`
- PTY support through ConPTY on Windows

Current Rust status:

- no Windows service implementation yet
- no Windows-specific PTY implementation yet
- no verified cross-platform runtime parity yet

## Utility Behavior Captured So Far

The following Go utility behavior has been extracted and partly ported:

### SSH URL Parsing

Captured through:

- `compat/golden/runtime/ssh_url_ipv4.json`
- `compat/golden/runtime/ssh_url_empty_host.json`
- `compat/golden/runtime/ssh_url_ipv6.json`

Current Rust behavior:

- default username comes from `USER` or `USERNAME`, else `root`
- default host for empty host is `127.0.0.1`
- default port is `22`
- IPv6 hosts are normalized into bracketed form when needed

### Endpoint Formatting

Current Rust behavior:

- `Endpoint` displays as `host:port`

### SSH Config Parsing

Captured through:

- `compat/golden/runtime/ssh_config.json`
- input fixture `pkg/utils/testdata/ssh_config`

Current Rust parser behavior:

- parses `Host` blocks
- ignores `Host *` entries as nodes
- supports fields:
  - `HostName`
  - `Port`
  - `User`
  - `IdentityFile`
  - `UserKnownHostsFile`
  - `StrictHostKeyChecking`
  - `ProxyJump`
- ignores unknown keys

### Known Hosts Formatting

Current Rust behavior:

- creates the file lazily if it does not exist
- appends one line per enrolled key
- uses `host key` format for non-IPv6 default-port hosts
- uses `[host]:port key` format for non-default ports and bracketed hosts

## Validation Performed So Far

### Baseline Capture

The repository includes a reproducible baseline capture script:

- `scripts/capture_go_baselines.sh`

What it does:

- builds a temporary Go baseline binary at `/tmp/rospo-go-baseline`
- captures CLI outputs and exit codes
- serializes config parsing results
- serializes SSH URL parsing results
- serializes SSH config parsing results

### Go Test Validation

Previously executed and confirmed:

- `go test ./pkg/rio ./pkg/sshc ./pkg/sshd ./pkg/tun`

Verbose traces captured:

- `compat/golden/runtime/go_test_pkg_sshc.txt`
- `compat/golden/runtime/go_test_pkg_sshd.txt`
- `compat/golden/runtime/go_test_pkg_tun.txt`

### Rust Test Validation

Previously executed and confirmed:

- `cargo test --manifest-path rust/Cargo.toml`

Covered today by automated tests:

- root help output
- root no-arg output
- all captured command help outputs
- template output
- keygen output shape
- keygen stored-file behavior
- YAML config parsing for Go fixtures
- SSH URL parsing
- endpoint formatting
- SSH config parsing
- public-key serialization shape
- known-hosts line format

### Live Go/Rust Interoperability Checks

Observed successful interop:

- Rust `grabpubkey` against live Go `sshd`
- Rust `shell` against live Go `sshd` with public-key auth
- remote command `echo test` executed successfully
- Go server logs confirmed successful key authentication and exit status `0`

Observed incomplete or mismatching interop:

- Rust `shell` against a Go `sshd` instance started with disabled auth did not successfully complete
- Go server logged `ssh: no authentication methods available`
- Rust side returned `Channel send error`
- this path is not yet compatible

## Known Compatibility Traps

- Cobra help text ordering and wording are compatibility-sensitive
- root no-arg exit code is `0` despite printing an error and usage
- `run` in Go appears to contain a likely bug in the dedicated DNS proxy SSH client selection path:
  - it uses `conf.SocksProxy.SshClientConf`
  - not `conf.DnsProxy.SshClientConf`
- the migration has not yet decided whether that bug must be preserved
- modern Go `for range maxWorkers` behavior in the worker pool is valid and intentional

## Current Rust Coverage Summary

Implemented today in Rust:

- executable crate entrypoint in `rust/src/main.rs`
- module exports in `rust/src/lib.rs`
- fixture-driven CLI help and root-output matching
- fixture-driven template output
- config schema mirror and YAML loading
- SSH URL parsing
- endpoint formatting
- SSH config file parsing
- known-hosts formatting and appending
- `keygen`
- `grabpubkey`
- `shell`
- compatibility tests for CLI, config, and utility behavior

Not implemented yet in Rust:

- `dns-proxy` runtime
- `get` runtime
- `put` runtime
- `revshell` runtime
- `run` component orchestration
- `socks-proxy` runtime
- `sshd` runtime
- `tun forward`
- `tun reverse`
- reconnect loops
- Go-compatible keepalive logic
- SSH config integration into runtime connections
- jump-host routing
- SFTP client/server runtime
- SOCKS runtime
- DNS proxy runtime
- logging parity
- Windows service mode
- ConPTY or equivalent Windows PTY support
- full side-by-side Go/Rust behavioral diff suite
