# Rospo Migration Spec

Date: 2026-03-20

This document is the current migration spec for the Go-to-Rust rewrite of `rospo`. It is both a compatibility contract and a status snapshot of what is already implemented in the Rust tree.

This is not a claim that the Rust binary is fully equivalent yet.

## Scope

The target remains a drop-in Rust replacement with:

- identical command names
- identical flags and defaults
- semantically equivalent CLI parsing, flags, defaults, and help coverage
- identical config schema and config-loading behavior
- identical runtime behavior for SSH, tunnels, SOCKS, DNS proxying, SFTP, logging, and exit codes
- Go implementation artifacts remain useful as migration references and regression oracles, but mixed Go/Rust interoperability is no longer a required target

## Source Of Truth

Repository artifacts currently used as compatibility oracles:

- Go contract summary: `docs/migration/go_inventory.md`
- Interim migration report: `docs/migration/report.md`
- Architecture snapshot: `ARCHITECTURE.md`
- Design decision log: `DECISIONS.md`
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
  - `rust/tests/behavioral_diff.rs`
  - `rust/tests/keepalive_compat.rs`
- Additional platform verification commands used during migration:
  - `AWS_LC_SYS_NO_ASM=1 cargo check --manifest-path rust/Cargo.toml --target x86_64-pc-windows-gnu`

Recent implementation notes reflected in the current Rust tree:

- config booleans accept YAML 1.1-style string spellings such as `yes`, `no`, `on`, and `off`
- the reorganized Rust modules now include targeted comments in dense runtime, PTY, config, progress, and precedence code paths where intent would otherwise be harder to infer
- SFTP recovery now uses a shared reconnect coordinator per client so one outage triggers one recovery loop rather than one reconnect attempt per chunk worker
- interactive SFTP progress rendering now coordinates with normal log output so reconnect logs clear and restore the active transfer overlay instead of leaving stacked bar lines behind
- the Windows-specific Rust code now passes a GNU-target compile check; the remaining Windows gaps are runtime and toolchain validation rather than unresolved Rust source errors

## CLI Contract

### Root Command

- Binary name: `rospo`
- Persistent global flag:
  - `-q, --quiet`
  - default: `false`
- Current Rust CLI implementation:
  - uses `clap` derive parsing from `rust/src/cli/app.rs`
  - `rust/src/main.rs` invokes `Cli::parse()` on the normal CLI path
  - typed clap subcommands are dispatched directly to runtime handlers
- Current Rust behavior:
  - `rospo --help` prints clap-generated help and exits `0`
  - `rospo -h` prints clap-generated help and exits `0`
  - `rospo --version` prints the clap version string and exits `0`
  - `rospo` with no args prints root help and exits `0`
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

- help output is clap-generated
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

- help output is clap-generated
- runtime is implemented
- SFTP download is implemented
- recursive download is implemented
- resumable single-stream download is implemented
- chunked concurrent single-file download is implemented
- bounded concurrent recursive download scheduling is implemented
- recursive download preserves the remote root directory name
- resumed transfer progress accounting is implemented
- downloaded file permissions are applied locally
- Go-style worker-pool/retry semantics are implemented
- interrupted transfers reconnect and resume instead of spinning or failing on the first reconnect error
- reconnect attempts are coordinated across workers to avoid per-chunk reconnect storms
- progress rendering is suppressed on non-terminal stdout to match the Go binary's captured-output behavior
- interactive terminal rendering now behaves like a single coordinated overlay instead of append-only status lines, but exact mpb-style output parity is not yet proven

#### `grabpubkey host:port`

Flags:

- `-k, --known-hosts`
  - default: `$HOME/.ssh/known_hosts`

Purpose in Go:

- connects to the SSH server
- accepts the host key without failing verification
- appends the host key to the known-hosts file

Rust status:

- help output is clap-generated
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

- help output is clap-generated
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

- help output is clap-generated
- runtime is implemented
- SFTP upload is implemented
- recursive upload is implemented
- resumable single-stream upload is implemented
- chunked concurrent single-file upload is implemented
- bounded concurrent recursive upload scheduling is implemented
- resumed transfer progress accounting is implemented
- uploaded file permissions are pushed to the remote target
- Go-style worker-pool/retry semantics are implemented
- interrupted transfers reconnect and resume instead of spinning or failing on the first reconnect error
- reconnect attempts are coordinated across workers to avoid per-chunk reconnect storms
- progress rendering is suppressed on non-terminal stdout to match the Go binary's captured-output behavior
- interactive terminal rendering now behaves like a single coordinated overlay instead of append-only status lines, but exact mpb-style output parity is not yet proven

#### `revshell [user@]host[:port]`

Flags:

- shared SSH client flags
- shared SSH server flags
- `-r, --remote`
  - default: `127.0.0.1:2222`

Purpose in Go:

- starts a local embedded SSH server and exposes it remotely through a reverse tunnel

Rust status:

- help output is clap-generated
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

- help output is clap-generated
- YAML file loading/parsing is implemented
- empty config behavior matches the captured placeholder output:
  - `2026/03/19 00:00:00 nothing to run`
- configured `sshd`, `tunnel`, `socksproxy`, and `dnsproxy` sections are started
- per-section nested `sshclient` blocks are supported where applicable
- exact Go startup/shutdown ordering and failure propagation are not yet proven equivalent

## Behavioral Diff Coverage

Current automated side-by-side binary and runtime diff coverage includes:

- Rust binary vs Go binary:
  - `grabpubkey` against the same Rust `sshd`
  - `shell` exec against the same Rust `sshd`
  - `put` against the same Rust `sshd`
  - `get` against the same Rust `sshd`
- Rust `sshd` vs Go `sshd`:
  - shell exit-status probe
  - SFTP roundtrip probe
  - SOCKS proxy probe
  - forward tunnel probe
  - reverse tunnel probe

The current side-by-side suite has already forced and verified these Rust parity fixes:

- `grabpubkey` now writes Go-compatible `known_hosts` host tokens even when the CLI server argument includes `user@`
- global `-q` no longer suppresses the SSH banner; `-b/--disable-banner` controls banner suppression
- SFTP progress redraw output is no longer emitted when stdout is not a terminal

## Logging And Output Rules

Current Rust behavior:

- runtime logs use Go-style timestamps and component prefixes
- root `-q/--quiet` suppresses runtime logger output
- SSH auth banners remain controlled by `-b/--disable-banner`, not by root quiet mode
- captured non-interactive `get` and `put` output no longer includes terminal redraw progress noise

Current status:

- representative side-by-side binary output parity is covered for `grabpubkey`, `shell`, `get`, and `put`
- broad runtime wording parity has been aligned for SSH, tunnels, SOCKS, DNS, and SFTP
- exhaustive byte-for-byte output parity across every command and failure path is still not fully proven

#### `shell [user@]host[:port] [cmd_string]`

Purpose in Go:

- connects over SSH
- either runs an interactive shell or executes a command
- validates host keys unless insecure mode disables that
- authenticates using key and/or password
- prints server banners unless disabled

Rust status:

- help output is clap-generated
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
  - client-side raw-terminal handling on Unix
  - immediate local shell teardown on remote exit without requiring extra keypresses
  - root/global quiet-mode suppression for client-side banner and runtime logging
- remaining gaps:
  - no interactive password prompt workflow beyond direct flag use
  - Windows interactive client-side raw-terminal parity is not yet validated
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

- help output is clap-generated
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

- help output is clap-generated
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

- help output is clap-generated
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

- help output is clap-generated
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
- YAML boolean-like strings such as `yes`, `no`, `on`, and `off` are accepted for boolean fields
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
- YAML 1.1-style boolean spellings such as `forward: yes`

## Runtime Behavior Rules To Preserve

### SSH Client

Go behavior that must still be preserved exactly:

- reconnect loop retries every 5 seconds where the Go service-style commands do so
- host-key failure wording and exit codes
- lazy known-hosts creation behavior in all edge cases
- banner behavior in all code paths

Current Rust status:

- real transport exists in the `rust/src/ssh/` module family
- SSH config integration exists
- jump-host routing exists
- known-hosts verification exists
- untrusted-host failures now return an explicit trust error that points users at `rospo grabpubkey`
- malformed `known_hosts` files are detected and reported explicitly
- command and interactive shell paths exist
- Unix client-side raw-terminal handling exists for interactive shells
- Unix client-side PTY resize propagation exists
- interactive password fallback exists when the CLI did not receive `-p/--password`
- `authenticate_none` fallback is covered for no-auth servers
- stdout/stderr separation and exit-status propagation are covered for the CLI shell path
- service-style reconnect loops exist for tunnels
- root/global quiet mode is wired into client-side runtime output suppression
- stock `russh` keepalive/ping behavior is used
- client keepalive behavior is regression-tested against the Rust `sshd`
- post-disconnect keepalive attempts now fail immediately on the local client side

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
- listener lifecycle and shutdown semantics

Current Rust status:

- forward and reverse runtimes exist
- reconnect loop exists
- echo-path behavior is validated
- client keepalive loop uses stock `russh` ping behavior with a 5-second interval

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
- recursive `get` preserves the remote root directory name
- per-file resumed progress accounting exists for upload/download
- local and remote permission preservation is implemented for transferred files
- Go-style per-chunk retry worker behavior is implemented
- exact mpb progress-bar output and mixed-version SFTP validation are not yet fully proven

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

- clap-generated root help and root no-arg output
- clap-generated command help availability across implemented commands
- malformed invocation coverage for representative clap error paths
- template output
- keygen output shape and stored-file behavior
- Go config fixture parsing and config file failure behavior
- config boolean compatibility for YAML values like `yes` and `no`
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
- unknown-host trust failure behavior
- malformed `known_hosts` parse failure behavior
- password-auth SSH connect
- CLI password-prompt fallback when `-p/--password` is omitted
- no-auth SSH connect through `authenticate_none`
- jump-host SSH connect
- shell-disabled rejection
- shell CLI stdout/stderr separation and exit-code propagation
- keepalive failure after explicit client disconnect
- SOCKS end-to-end proxying
- SFTP enabled/disabled subsystem behavior
- large-file chunked SFTP upload/download roundtrip
- recursive SFTP download root-directory layout and permission preservation
- resumed SFTP upload/download progress offset accounting
- forward tunnel echo path
- reverse tunnel echo path
- Rust keepalive request success against Rust `sshd`
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

## Maintainability Status

Current Rust maintainability work completed so far:

- early monolithic `mod.rs` files were split into package-style focused modules
- targeted comments were added where behavior is non-obvious, especially in:
  - CLI/config precedence resolution
  - interactive client terminal handling
  - Unix PTY and Windows ConPTY server process wiring
  - SFTP progress rendering and worker scheduling
  - authorized-keys loading and config compatibility shims
- Rust `put` and `get` against Go `sshd` SFTP
- Rust `socks-proxy` against Go `sshd`
- Rust `dns-proxy` against Go `sshd`
- Rust `revshell` against Go `sshd`
- Rust `grabpubkey`, `shell`, `put`, `get`, forward tunnel, and reverse tunnel against Rust `sshd`

Remaining interop gap:

- full mixed-version matrix coverage is still incomplete, but mixed-version interoperability is no longer a required migration target

## Known Compatibility Traps

- clap now owns help generation; exact Go/Cobra wording is no longer the target
- root no-arg exit code is `0` despite printing an error and usage
- `run` in Go appears to contain a likely bug in DNS proxy SSH-client selection
- logging/output parity is still a major unfinished area
- Windows behavior is implemented but not yet parity-validated on a real Windows host
- the Rust tree now uses upstream `russh` without a local patch override

## Current Rust Coverage Summary

Implemented in Rust:

- executable crate entrypoint and module layout
- maintainable package-style Rust layout with focused submodules for `cli`, `sshd`, `ssh`, `sftp`, and `utils`
- clap-driven CLI parsing and help rendering
- fixture-driven template output only
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
- Unix client-side raw-terminal handling and non-blocking interactive shell teardown
- `socks-proxy`
- `dns-proxy`
- `get`
- `put`
- chunked concurrent single-file SFTP upload/download
- bounded concurrent recursive SFTP scheduling
- Go-style per-chunk retry worker behavior for SFTP transfers
- resumed SFTP progress accounting
- recursive SFTP `get` root-directory preservation
- transferred-file permission preservation for SFTP upload/download
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
- stock upstream `russh` keepalive/ping behavior without a vendored dependency patch

Not implemented or not yet fully equivalent:

- exact Go logging/output parity
- exhaustive Cobra failure/exit-code parity for all malformed invocations
- exact Go mpb progress-bar formatting/output equivalence
- full side-by-side Go/Rust behavioral diff suite
- validated Windows service parity
- validated Windows ConPTY/PTY parity
- exhaustive mixed Go/Rust interoperability coverage is not pursued as a required outcome
- standalone Rust equivalents for Go-only helper packages `pkg/registry`, `pkg/worker`, and `pkg/rio`
