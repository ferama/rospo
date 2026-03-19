# Rospo Migration Architecture

Date: 2026-03-19

This document describes the project structure and responsibilities as they exist today across:

- the original Go implementation
- the migration support tooling added during this rewrite effort
- the current Rust port

## Repository-Level Layout

### Original Go Project

Primary Go areas:

- `cmd`
  - Cobra command tree and flag definitions
- `pkg/conf`
  - YAML config loading
- `pkg/sshc`
  - SSH client behavior
- `pkg/sshd`
  - embedded SSH server
- `pkg/tun`
  - tunnel engine
- `pkg/utils`
  - parsing and helper utilities
- `pkg/rio`
  - bidirectional stream copy helpers
- `pkg/rpty`
  - PTY abstraction
- `pkg/logger`
  - logging
- `pkg/worker`
  - worker pool
- `pkg/registry`
  - object registry

### Migration Tooling Added So Far

- `docs/migration/go_inventory.md`
  - manually curated Go contract summary
- `docs/migration/report.md`
  - interim migration progress report
- `scripts/capture_go_baselines.sh`
  - reproducible golden-fixture capture script
- `tools/go_baseline/main.go`
  - helper binary to serialize Go runtime/parser behavior to JSON
- `pkg/utils/ssh_config_parser_baseline.go`
  - narrow adapter exposing Go SSH config parsing to baseline tooling
- `compat/golden/cli`
  - captured Go CLI stdout/stderr outputs and exit codes
- `compat/golden/runtime`
  - captured Go config parsing, SSH URL parsing, SSH config parsing, and package test traces

### Rust Port Added So Far

- `rust/Cargo.toml`
  - Rust crate definition and dependency set
- `rust/src/main.rs`
  - binary entrypoint
- `rust/src/lib.rs`
  - module exports
- `rust/src/cli/mod.rs`
  - command dispatch and implemented CLI/runtime slices
- `rust/src/config/mod.rs`
  - YAML schema mirror
- `rust/src/utils/mod.rs`
  - utility behavior port
- `rust/src/ssh/mod.rs`
  - current SSH client transport slice
- `rust/src/logging/mod.rs`
  - placeholder logging module
- `rust/src/tunnel/mod.rs`
  - placeholder tunnel module
- `rust/src/socks/mod.rs`
  - placeholder SOCKS module
- `rust/src/sftp/mod.rs`
  - placeholder SFTP module
- `rust/tests/cli_compat.rs`
  - CLI regression tests against Go fixtures
- `rust/tests/config_compat.rs`
  - config parsing regression tests
- `rust/tests/utils_compat.rs`
  - utility regression tests

## Go Module Responsibilities

This is the module decomposition extracted from the Go codebase and captured in the migration inventory.

### `cmd`

Responsibilities:

- defines the Cobra command tree
- defines all command names and aliases
- defines shared flags and per-command flags
- defines help text, usage text, and default values
- routes execution into package-level runtime modules

### `pkg/conf`

Responsibilities:

- loads YAML config files into composed runtime structs
- preserves permissive YAML behavior
- exposes config to runtime entrypoints like `run`

### `pkg/sshc`

Responsibilities:

- SSH client connectivity
- remote shell execution
- host-key handling
- SFTP client operations
- SOCKS proxy client path
- DNS proxy client path
- reconnect and keepalive behavior

### `pkg/sshd`

Responsibilities:

- embedded SSH server
- session/shell handling
- PTY/session management
- authentication
- SFTP subsystem
- remote forwarding support

### `pkg/tun`

Responsibilities:

- forward tunnels
- reverse tunnels
- reconnect loop
- tunnel liveness and metrics

### `pkg/utils`

Responsibilities:

- SSH URL parsing
- endpoint formatting
- SSH config parsing
- known-hosts helpers
- key generation helpers
- shell lookup and environment-specific helpers

### `pkg/rio`

Responsibilities:

- bidirectional stream copy
- optional byte counters / transfer accounting

### `pkg/rpty`

Responsibilities:

- PTY abstraction on Unix and Windows
- Unix PTY via `creack/pty`
- Windows PTY via ConPTY

### `pkg/logger`

Responsibilities:

- colorized logger creation
- global enable/disable behavior
- formatting consistency across subsystems

### `pkg/worker`

Responsibilities:

- bounded worker pool used by SFTP chunk transfer flows

### `pkg/registry`

Responsibilities:

- in-memory object registry used by the tunnel layer

## Migration Support Architecture

### Baseline Capture Flow

`scripts/capture_go_baselines.sh` is the main migration capture pipeline.

Responsibilities:

- build a temporary Go binary at `/tmp/rospo-go-baseline`
- run a fixed set of CLI invocations
- capture stdout/stderr and exit codes for each invocation
- invoke `tools/go_baseline` for structured JSON snapshots

### `tools/go_baseline/main.go`

Responsibilities:

- wrap existing Go runtime helpers in a narrow serialization tool
- expose modes:
  - `config`
  - `ssh-url`
  - `ssh-config`
- pretty-print JSON to stdout for fixture generation

### `pkg/utils/ssh_config_parser_baseline.go`

Responsibilities:

- expose the existing Go SSH config parser through a minimal adapter
- keep migration tooling from having to duplicate parser logic

### Golden Fixture Layout

`compat/golden/cli` contains:

- one `.txt` file per captured CLI output
- one `.exitcode` file per captured CLI invocation

`compat/golden/runtime` contains:

- JSON baselines for config parsing
- JSON baselines for SSH URL parsing
- JSON baselines for SSH config parsing
- captured verbose Go test traces for networked package behavior

## Rust Module Responsibilities

### `rust/src/main.rs`

Responsibilities:

- process entrypoint
- calls `rospo::cli::run(std::env::args_os())`
- exits with the returned exit code

### `rust/src/lib.rs`

Responsibilities:

- exports the current Rust module surface:
  - `cli`
  - `config`
  - `logging`
  - `socks`
  - `sftp`
  - `ssh`
  - `tunnel`
  - `utils`

### `rust/src/cli/mod.rs`

Current responsibilities:

- parse argument vectors manually
- dispatch to implemented commands
- return a structured `CliResponse`
- print stdout/stderr in `run`
- supply fixture-backed help output
- supply fixture-backed root no-arg output
- supply fixture-backed template output
- implement:
  - `keygen`
  - `grabpubkey`
  - `shell`
  - partial `run`

Important internal concepts:

- `CliResponse`
  - carries `stdout`, `stderr`, and `exit_code`
- `execute`
  - test-friendly pure-ish entrypoint over argument vectors
- `run`
  - user-facing entrypoint that prints and returns exit code
- `dispatch`
  - routes command names to handlers
- `parse_ssh_client_command`
  - shared flag parsing for SSH-client-oriented commands

Current architectural choice:

- the CLI is not currently built through `clap` runtime parsing
- it uses manual dispatch plus Go fixtures to lock output compatibility first

### `rust/src/config/mod.rs`

Responsibilities:

- define Rust structs mirroring the Go YAML schema
- preserve field names with `serde(rename = "...")`
- preserve missing-field behavior via `default` and `Option`
- provide `load_config`

Types defined:

- `Config`
- `JumpHostConf`
- `SshClientConf`
- `TunnelConf`
- `SocksProxyConf`
- `DnsProxyConf`
- `SshdConf`

### `rust/src/utils/mod.rs`

Responsibilities:

- runtime/environment helpers:
  - `current_username`
  - `current_home_dir`
  - `expand_user_home`
- SSH address helpers:
  - `SshUrl`
  - `Endpoint`
  - `parse_ssh_url`
  - `new_endpoint`
- SSH config parsing:
  - `NodeConfig`
  - `parse_ssh_config_content`
  - `parse_ssh_config_file`
- filesystem helpers:
  - `write_file_0600`
- public-key helpers:
  - `serialize_public_key`
  - `add_host_key_to_known_hosts`

Notable current limitation:

- these helpers exist independently; they are not yet integrated into all runtime paths such as jump-host resolution or full SSH config override application

### `rust/src/ssh/mod.rs`

Responsibilities today:

- define SSH-related constants:
  - `KEEPALIVE_REQUEST`
  - `CHECKALIVE_REQUEST`
- fetch server host keys:
  - `fetch_server_public_key`
- load private keys:
  - `load_secret_key`
- hold connection options:
  - `ClientOptions`
- establish SSH client sessions:
  - `Session::connect`
- execute commands:
  - `Session::run_command`
- request an interactive shell:
  - `Session::run_shell`
- disconnect:
  - `Session::disconnect`
- relay channel I/O:
  - `drain_channel`

Internal structure:

- `KeyGrabber`
  - minimal `russh::client::Handler` used for host-key capture
- `ClientHandler`
  - `russh::client::Handler` for runtime sessions
  - prints banners unless quiet
  - verifies server keys against known-hosts unless insecure

What this module does not yet own:

- jump-host chaining
- reconnect loops
- keepalive scheduling
- SFTP client
- server implementation
- reverse-forward request handling

### `rust/src/logging/mod.rs`

Responsibilities today:

- none beyond placeholder function shape

Intended responsibility later:

- reproduce Go logger formatting and quiet behavior

### `rust/src/tunnel/mod.rs`

Responsibilities today:

- define `RECONNECTION_INTERVAL_SECS = 5`

Intended responsibility later:

- forward tunnel implementation
- reverse tunnel implementation
- reconnection/liveness behavior

### `rust/src/socks/mod.rs`

Responsibilities today:

- define `DEFAULT_LISTEN_ADDRESS = "127.0.0.1:1080"`

Intended responsibility later:

- SOCKS4/SOCKS5 proxy runtime over SSH

### `rust/src/sftp/mod.rs`

Responsibilities today:

- define `DEFAULT_CHUNK_SIZE = 128 * 1024`

Intended responsibility later:

- SFTP client and server subsystem
- chunked concurrent transfer behavior

## Rust Test Architecture

### `rust/tests/cli_compat.rs`

Responsibilities:

- verify Rust CLI output equals captured Go fixtures
- cover root help
- cover root no-arg output
- cover all captured command help outputs
- cover template output
- cover keygen output shape
- cover keygen file storage behavior

### `rust/tests/config_compat.rs`

Responsibilities:

- verify Rust YAML parsing against Go config fixtures
- specifically cover:
  - secure SSH client config
  - insecure SSH client config
  - missing-boolean default behavior

### `rust/tests/utils_compat.rs`

Responsibilities:

- verify Rust utility behavior against Go JSON fixtures and expected string shapes
- cover:
  - SSH URL parsing
  - endpoint string formatting
  - SSH config parser output
  - public-key serialization format
  - known-hosts line formatting

## Current Control Flow

### CLI Execution Flow

1. `rust/src/main.rs` collects OS args.
2. `rospo::cli::run` calls `execute`.
3. `execute` converts args into owned strings.
4. `dispatch` routes based on the first non-binary arg.
5. Help and template paths use captured Go fixtures or the Go template file directly.
6. Implemented runtime commands invoke Rust handlers.
7. `CliResponse` stdout/stderr are printed and its exit code is returned.

### `grabpubkey` Flow

1. CLI parses `--known-hosts` plus the `host:port` argument.
2. `parse_ssh_url` resolves username/host/port shape.
3. A one-thread Tokio runtime is created inside the command handler.
4. `fetch_server_public_key` performs an SSH handshake and records the presented host key.
5. `add_host_key_to_known_hosts` appends a Go-compatible line to the chosen file.

### `shell` Flow

1. CLI parses shared SSH client flags manually.
2. CLI resolves default identity and known-hosts paths from the current home directory.
3. CLI builds `ClientOptions`.
4. A one-thread Tokio runtime is created inside the command handler.
5. `Session::connect` opens the SSH connection.
6. The handler performs host-key verification unless insecure.
7. Authentication is attempted in this order:
   - public key
   - password
   - none
8. If command arguments remain, `run_command` is used.
9. Otherwise, `run_shell` requests a PTY and shell.
10. `drain_channel` relays stdout/stderr and interactive stdin.
11. `disconnect` is attempted before returning.

## Dependency Architecture

Current Rust dependencies and intended use:

- `clap`
  - declared because it is part of the target migration stack
  - not yet used for live parsing
- `serde`
  - config struct serialization/deserialization
- `serde_yaml`
  - YAML parsing
- `tokio`
  - async runtime for SSH and later network services
- `russh`
  - SSH client today, SSH server later
- `tracing`
  - intended logging backend
- `tracing-subscriber`
  - intended logging formatting/configuration backend
- `internal-russh-forked-ssh-key`
  - SSH key parsing/serialization helpers compatible with `russh`
- `p521`
  - P-521 key generation
- `sec1`
  - SEC1 PEM output for private keys
- `serde_json` as dev dependency
  - JSON fixture parsing in tests

## Architectural State Summary

Stable and real today:

- Go project remains the compatibility oracle
- Go golden fixtures exist and are reproducible
- Rust has a real executable crate
- Rust has a real YAML schema mirror
- Rust has a real utility layer for parsed baseline behavior
- Rust has a real SSH client slice for `grabpubkey` and `shell`
- Rust tests assert fixture compatibility for several non-trivial behaviors

Still architectural placeholders:

- logging subsystem
- tunnel subsystem
- SOCKS subsystem
- SFTP subsystem
- embedded SSH server
- full config-driven runtime composition
- Windows service and PTY integration
