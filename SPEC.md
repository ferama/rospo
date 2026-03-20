# Rospo Migration Spec

Date: 2026-03-20

This document is the current migration spec for the Go-to-Rust rewrite of `rospo`. It is both a compatibility contract and a status snapshot of what is already implemented in the Rust tree.

This is not a claim that the Rust binary is fully equivalent yet.

## Scope

The target remains a drop-in Rust replacement with:

- identical command names
- identical flags and defaults
- identical help output
- identical config schema and config-loading behavior
- identical runtime behavior for SSH, tunnels, SOCKS, DNS proxying, SFTP, logging, and exit codes
- interoperability with the Go implementation in both directions

## Source Of Truth

Repository artifacts currently used as compatibility oracles:

- Go contract summary: `docs/migration/go_inventory.md`
- Interim migration report: `docs/migration/report.md`
- Go CLI golden fixtures: `compat/golden/cli`
- Go runtime/config/parser fixtures: `compat/golden/runtime`
- Go baseline capture script: `scripts/capture_go_baselines.sh`
- Go baseline helper: `tools/go_baseline/main.go`
- Rust compatibility and integration tests:
  - `rust/tests/cli_compat.rs`
  - `rust/tests/config_compat.rs`
  - `rust/tests/utils_compat.rs`
  - `rust/tests/keys_compat.rs`
  - `rust/tests/ssh_integration.rs`
  - `rust/tests/sshd_integration.rs`
  - `rust/tests/tunnel_integration.rs`
  - `rust/tests/interop_go_server.rs`

## CLI Contract

### Root Command

- Binary name: `rospo`
- Root long description: `The tool to create relieable ssh tunnels.`
- Persistent global flag:
  - `-q, --quiet`
  - default: `false`
- Cobra version flag is enabled in Go.
- Observed Go behaviors:
  - `rospo --help` prints help and exits `0`
  - `rospo -h` prints help and exits `0`
  - `rospo --version` prints version and exits `0`
  - `rospo` with no args prints Cobra arity error plus usage and exits `0`
- Current Rust status:
  - root `-q, --quiet` is accepted before subcommands and initializes global quiet-mode suppression for runtime logging

### Shared SSH Client Flags

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

Rust status:

- help output is fixture-matched
- runtime is implemented
- local UDP and TCP listeners are implemented
- upstream DNS-over-TCP framing over SSH is implemented
- `run` command integration is implemented

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

Rust status:

- help output is fixture-matched
- runtime is implemented
- SFTP download is implemented
- recursive download is implemented
- resumable single-stream download is implemented
- chunked concurrent single-file download is implemented
- bounded concurrent recursive download scheduling is implemented
- exact Go worker-pool/progress semantics are not yet proven equivalent

#### `grabpubkey host:port`

Flags:

- `-k, --known-hosts`
  - default: `$HOME/.ssh/known_hosts`

Purpose in Go:

- connects to the SSH server
- accepts the host key without failing verification
- appends the host key to the known-hosts file

Rust status:

- help output is fixture-matched
- implemented with a real SSH handshake
- appends known-hosts entries in Go-compatible format

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

Rust status:

- help output is fixture-matched
- implemented
- emits SEC1 PEM private keys and OpenSSH `ecdsa-sha2-nistp521` public keys

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

Rust status:

- help output is fixture-matched
- runtime is implemented
- SFTP upload is implemented
- recursive upload is implemented
- resumable single-stream upload is implemented
- chunked concurrent single-file upload is implemented
- bounded concurrent recursive upload scheduling is implemented
- exact Go worker-pool/progress semantics are not yet proven equivalent

#### `revshell [user@]host[:port]`

Flags:

- shared SSH client flags
- shared SSH server flags
- `-r, --remote`
  - default: `127.0.0.1:2222`

Purpose in Go:

- starts a local embedded SSH server and exposes it remotely through a reverse tunnel

Rust status:

- help output is fixture-matched
- implemented
- composes embedded Rust `sshd` with reverse tunnel runtime

#### `run config_file_path.yaml`

Purpose in Go:

- loads YAML config
- starts configured components:
  - SSH client-backed tunnels
  - embedded SSH server
  - SOCKS proxy
  - DNS proxy

Rust status:

- help output is fixture-matched
- YAML file loading/parsing is implemented
- empty config behavior matches the captured placeholder output:
  - `2026/03/19 00:00:00 nothing to run`
- configured `sshd`, `tunnel`, `socksproxy`, and `dnsproxy` sections are started
- per-section nested `sshclient` blocks are supported where applicable
- exact Go startup/shutdown ordering and failure propagation are not yet proven equivalent

#### `shell [user@]host[:port] [cmd_string]`

Purpose in Go:

- connects over SSH
- either runs an interactive shell or executes a command
- validates host keys unless insecure mode disables that
- authenticates using key and/or password
- prints server banners unless disabled

Rust status:

- help output is fixture-matched
- implemented on a real SSH client transport
- supports:
  - known-hosts verification unless insecure mode is enabled
  - key authentication
  - password authentication when provided
  - `authenticate_none` fallback
  - banner printing unless disabled
  - remote command execution
  - interactive shell request
  - SSH config host overrides
  - `ProxyJump` and explicit jump-host chains
  - Unix PTY-backed interactive sessions
  - PTY resize propagation on Unix
  - root/global quiet-mode suppression for client-side banner and runtime logging
- remaining gaps:
  - no interactive password prompt workflow beyond direct flag use
  - Windows interactive client-side PTY behavior is not implemented
  - exact edge-case parity for stderr/stdout/exit-status is not yet exhaustively proven

#### `socks-proxy [user@]host[:port]`

Flags:

- shared SSH client flags
- `-l, --listen-address`
  - default: `127.0.0.1:1080`

Purpose in Go:

- runs a local SOCKS proxy over SSH
- supports SOCKS4 and SOCKS5

Rust status:

- help output is fixture-matched
- runtime is implemented
- SOCKS4 and SOCKS5 handshake paths are implemented

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

Rust status:

- help output is fixture-matched
- runtime is implemented
- supports:
  - server key bootstrap/load
  - public-key auth
  - password auth
  - disabled-auth mode
  - shell and exec requests
  - disable-shell mode
  - frog banner with Windows suppression guard
  - SFTP subsystem and disable-SFTP-subsystem behavior
  - port forwarding and reverse forwarding
  - local-file and HTTP/HTTPS `authorized_keys` sources
  - Go-style `:2222` listen addresses
  - Unix PTY-backed shell sessions
  - Windows ConPTY-backed shell sessions
- note:
  - server-side keepalives are disabled to avoid OpenSSH auth-phase protocol errors

#### `template`

Purpose in Go:

- prints the bundled config template

Rust status:

- help output is fixture-matched
- runtime behavior is implemented
- reads `cmd/configs/config_template.yaml` and appends a trailing newline

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

Rust status:

- help output is fixture-matched
- forward and reverse runtimes are implemented
- reconnect loop uses a 5-second retry interval

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

Sequence of tunnel entries. Each entry contains:

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

Observed from Go and preserved in current Rust types:

- missing top-level sections decode as `nil` in Go and `None` in Rust
- missing booleans decode as `false`
- missing strings decode as empty string
- missing sequences decode as empty vectors when the containing struct is present
- unknown YAML keys are ignored
- `dnsproxy.remote_dns_address` is not defaulted at raw YAML load time
- defaults mainly live in CLI flag definitions and runtime constructors

Fixture caveat still preserved:

- `pkg/conf/testdata/sshd.yaml` uses `port`, but the actual schema field is `listen_address`
- the Go test using that file only validates `disable_shell` defaulting

Current Rust config coverage:

- `pkg/conf/testdata/sshc.yaml`
- `pkg/conf/testdata/sshc_insecure.yaml`
- `pkg/conf/testdata/sshc_secure_default.yaml`
- `pkg/conf/testdata/sshd.yaml`
- nonexistent config-file failure
- unparsable config-file failure

## Runtime Behavior Rules To Preserve

### SSH Client

Go behavior that must still be preserved exactly:

- reconnect loop retries every 5 seconds where the Go service-style commands do so
- keepalive request `keepalive@rospo` behavior and timing
- host-key failure wording and exit codes
- lazy known-hosts creation behavior in all edge cases
- banner behavior in all code paths

Current Rust status:

- real transport exists in `rust/src/ssh/mod.rs`
- SSH config integration exists
- jump-host routing exists
- known-hosts verification exists
- command and interactive shell paths exist
- service-style reconnect loops exist for tunnels
- root/global quiet mode is wired into client-side runtime output suppression
- exact Go keepalive behavior is not yet proven equivalent

### SSH Server

Go behavior that must still be preserved exactly:

- session behavior and request lifecycle
- auth and no-auth behavior
- banner behavior
- disable-shell and disable-subsystem behavior
- forwarding behavior
- logging wording and timing

Current Rust status:

- embedded SSH server exists
- key auth, password auth, disable-auth, SFTP, shell/exec, forwarding, and HTTP/HTTPS `authorized_keys` exist
- Unix PTY sessions exist
- Windows ConPTY PTY path exists under `cfg(windows)`
- Windows service entrypoint exists under `cfg(windows)`
- Windows runtime parity is not yet validated on a Windows host

### Tunnels

Go behavior that must still be preserved exactly:

- reconnect every 5 seconds
- forward uses SSH `direct-tcpip`
- reverse uses `tcpip-forward` and `forwarded-tcpip`
- server-side liveness behavior using `checkalive@rospo`
- listener lifecycle and shutdown semantics

Current Rust status:

- forward and reverse runtimes exist
- reconnect loop exists
- echo-path behavior is validated
- exact `checkalive@rospo` parity is not yet proven

### SOCKS Proxy

Go behavior that must still be preserved exactly:

- SOCKS4 support
- SOCKS5 support
- unsupported-mode behavior and failure semantics

Current Rust status:

- SOCKS4 and SOCKS5 runtimes exist
- end-to-end proxying is validated

### DNS Proxy

Go behavior that must still be preserved exactly:

- local UDP listener
- local TCP listener
- upstream DNS-over-TCP framing
- default remote DNS server `1.1.1.1:53`

Current Rust status:

- UDP and TCP listeners exist
- upstream framing exists
- `run` integration exists

### SFTP

Go behavior that must still be preserved exactly:

- resumable upload/download
- bounded worker pools
- chunked concurrent transfers
- recursive transfer support
- embedded server subsystem support

Current Rust status:

- client upload/download exist
- recursive transfer exists
- resumable single-stream logic exists
- embedded server subsystem exists
- chunked concurrent single-file transfer logic exists
- bounded concurrent recursive transfer scheduling exists
- exact Go worker-pool/progress behavior is not yet proven equivalent

### Logging

Go behavior that must still be preserved exactly:

- colorized `log.Logger` output
- global quiet handling
- command/runtime log prefixes and formatting
- stdout/stderr placement

Current Rust status:

- `rust/src/logging/mod.rs` is implemented
- the Rust logger writes Go-style `LstdFlags` timestamps to stdout
- ANSI-colored prefixes are emitted on terminals
- global quiet suppression is implemented and wired through root `-q`
- runtime logging is wired through the SSH client, embedded SSH server, and tunnel engine
- exact wording, per-path coverage, and stdout/stderr placement are not yet fully matched to Go

### Windows / Cross-Platform

Go behavior that must still be preserved exactly:

- Linux support
- macOS support
- Windows support
- Windows service mode via `go-svc`
- PTY support through ConPTY on Windows

Current Rust status:

- Unix paths are exercised
- Windows service mode is implemented behind `cfg(windows)`
- Windows PTY/ConPTY behavior is implemented behind `cfg(windows)`
- this implementation was not live-validated on Windows from the current host
- cross-platform parity is not yet verified

## Utility Behavior Captured So Far

The following Go utility behavior has been extracted and ported or partially ported:

- SSH URL parsing
- endpoint formatting
- SSH config parsing
- known-hosts entry formatting
- home expansion
- default-shell lookup fallback
- byte-size formatting
- private-key loading

## Validation Performed So Far

### Baseline Capture

The repository includes `scripts/capture_go_baselines.sh`, which captures:

- CLI outputs and exit codes
- config parsing results
- SSH URL parsing results
- SSH config parsing results

### Go Test Validation

Previously executed and captured:

- `go test ./pkg/rio ./pkg/sshc ./pkg/sshd ./pkg/tun`

Verbose traces captured:

- `compat/golden/runtime/go_test_pkg_sshc.txt`
- `compat/golden/runtime/go_test_pkg_sshd.txt`
- `compat/golden/runtime/go_test_pkg_tun.txt`

### Rust Test Validation

Automated Rust coverage currently includes:

- root help and root no-arg output
- all captured command help outputs
- template output
- keygen output shape and stored-file behavior
- Go config fixture parsing and config file failure behavior
- SSH URL parsing
- endpoint formatting
- SSH config parsing
- home expansion
- default-shell fallback
- byte-size formatting
- identity-file loading
- public-key serialization shape
- known-hosts line format
- secure SSH connect with known-hosts verification
- password-auth SSH connect
- jump-host SSH connect
- shell-disabled rejection
- SOCKS end-to-end proxying
- SFTP enabled/disabled subsystem behavior
- large-file chunked SFTP upload/download roundtrip
- forward tunnel echo path
- reverse tunnel echo path
- automated Rust client -> Go server interop for shell
- automated Rust client -> Go server interop for SFTP upload/download
- automated Rust client -> Go server interop for forward tunnel
- automated Rust client -> Go server interop for reverse tunnel
- Unix-only PTY shell validation against Rust `sshd`

Windows validation gap:

- Windows-target compile/runtime validation could not be completed from the current macOS host because Cargo could not fetch Windows-target dependencies in the sandboxed no-network environment

### Live Go/Rust Interoperability Checks

Observed successful interop:

- Rust `grabpubkey` against Go `sshd`
- Rust `shell` against Go `sshd`
- Rust `tun forward` against Go `sshd`
- Rust `tun reverse` against Go `sshd`
- Rust `put` and `get` against Go `sshd` SFTP
- Rust `socks-proxy` against Go `sshd`
- Rust `dns-proxy` against Go `sshd`
- Rust `revshell` against Go `sshd`
- Rust `grabpubkey`, `shell`, `put`, `get`, forward tunnel, and reverse tunnel against Rust `sshd`

Remaining interop gap:

- full mixed-version matrix coverage is still incomplete

## Known Compatibility Traps

- Cobra help text ordering and wording remain compatibility-sensitive
- root no-arg exit code is `0` despite printing an error and usage
- `run` in Go appears to contain a likely bug in DNS proxy SSH-client selection
- logging/output parity is still a major unfinished area
- Windows behavior is still incomplete

## Current Rust Coverage Summary

Implemented in Rust:

- executable crate entrypoint and module layout
- fixture-driven CLI help and root-output matching
- fixture-driven template output
- root `-q, --quiet` acceptance and global quiet-mode initialization
- config schema mirror and YAML loading
- config file loading behavior
- SSH URL parsing
- endpoint formatting
- SSH config file parsing
- home expansion
- default-shell lookup fallback
- byte-size formatting
- known-hosts formatting and appending
- `keygen`
- `grabpubkey`
- `shell`
- `socks-proxy`
- `dns-proxy`
- `get`
- `put`
- chunked concurrent single-file SFTP upload/download
- bounded concurrent recursive SFTP scheduling
- `tun forward`
- `tun reverse`
- `sshd`
- `revshell`
- non-placeholder `run` orchestration for implemented subsystems
- HTTP/HTTPS `authorized_keys` loading
- Unix PTY-backed interactive shell support in the embedded server
- Windows service entrypoint and SCM detection
- Windows ConPTY-backed PTY support in the embedded server
- Go-style stdout logger with timestamps, prefixes, ANSI colors, and quiet suppression
- automated Rust compatibility/integration tests for the implemented areas
- automated Rust -> Go server interoperability tests for shell, SFTP, and tunnels

Not implemented or not yet fully equivalent:

- exact Go logging/output parity
- exact Cobra failure/exit-code parity for all malformed invocations
- exact Go worker-pool/progress SFTP equivalence
- full side-by-side Go/Rust behavioral diff suite
- validated Windows service parity
- validated Windows ConPTY/PTY parity
- exhaustive mixed Go/Rust interoperability coverage
- standalone Rust equivalents for Go-only helper packages `pkg/registry`, `pkg/worker`, and `pkg/rio`
