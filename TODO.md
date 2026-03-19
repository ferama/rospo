# Rospo Migration TODO

Date: 2026-03-19

This is the exhaustive remaining-work and incomplete-implementation list based on the current repository state.

## Completion Status

The migration is not complete. The Rust binary is not yet a drop-in replacement.

Implemented in Rust:

- fixture-matched root help and root no-arg output
- fixture-matched command help outputs for all captured commands
- fixture-matched template output
- config schema mirror and YAML parsing
- utility behavior for SSH URL parsing, endpoint formatting, SSH config parsing, known-hosts formatting
- `keygen`
- `grabpubkey`
- partial `shell`
- partial `run`
- compatibility tests for the above

Not complete:

- almost all runtime features outside `keygen`, `grabpubkey`, and part of `shell`

## CLI Parity Work

- implement full runtime behavior for:
  - `dns-proxy`
  - `get`
  - `put`
  - `revshell`
  - `run`
  - `socks-proxy`
  - `sshd`
  - `tun forward`
  - `tun reverse`
- verify exact exit codes for all success and failure paths, not just help and current implemented slices
- verify unknown-flag and malformed-invocation behavior against Cobra for every command
- verify `help` subcommand parity beyond the currently captured combinations
- decide whether to keep manual parsing or migrate live parsing to `clap` without losing exact Cobra compatibility
- wire the root `-q/--quiet` behavior through all commands and runtime paths

## Config Layer Work

- validate all Go config fixtures, not only the currently covered SSH client fixtures
- add Rust tests for:
  - `pkg/conf/testdata/sshd.yaml`
  - tunnel-oriented configs
  - socks proxy configs
  - DNS proxy configs
  - mixed multi-section configs used by `run`
- verify unknown-field ignoring matches Go in every relevant scenario
- implement runtime defaulting behavior outside raw YAML load
- decide and document whether the apparent Go `run` DNS proxy config bug must be preserved:
  - Go appears to use `conf.SocksProxy.SshClientConf`
  - expected field would be `conf.DnsProxy.SshClientConf`

## SSH Client Work

- implement reconnect loop with 5-second retry cadence
- implement periodic `keepalive@rospo` sending every 5 seconds
- verify host-key failure messaging matches Go wording and exit codes
- integrate parsed SSH config overrides into connection setup:
  - user
  - hostname
  - port
  - identity file
  - user-known-hosts file
  - strict host key checking
  - proxy jump
- implement actual `--jump-host` routing
- implement config-driven jump-host chains from YAML `jump_hosts`
- implement banner behavior exactly for all code paths
- verify password-auth behavior and prompts against Go
- verify no-auth server behavior against Go
- fix the currently observed no-auth mismatch:
  - Rust `shell` against Go `sshd -T` failed
  - Go logged `ssh: no authentication methods available`
  - Rust returned `Channel send error`
- implement PTY resize propagation
- verify interactive shell terminal-mode behavior against Go
- verify stdout/stderr interleaving and exit-status propagation
- verify identity-file loading behavior including passphrases if Go supports them
- verify known-hosts creation behavior on malformed files, missing parents, and parse failures

## Embedded SSH Server Work

- implement Rust `sshd`
- support key-based authentication
- support password authentication
- support disabled-auth mode
- support shell/session requests
- support disable-shell mode
- implement frog banner behavior with Windows suppression rules
- implement SFTP subsystem support
- implement disable-SFTP-subsystem option
- implement remote port forwarding support
- implement disable-tunnelling option
- implement configurable shell executable
- verify interoperability:
  - Go client -> Rust server
  - Rust client -> Rust server
  - Rust client -> Go server
  - Go client -> Go server as baseline

## Tunnel Engine Work

- implement forward tunnel runtime
- implement reverse tunnel runtime
- implement reconnect loop with 5-second retry
- implement `direct-tcpip` flow for forward tunnels
- implement `tcpip-forward` requests for reverse tunnels
- implement `forwarded-tcpip` accept handling
- implement server-side liveness behavior using `checkalive@rospo`
- match Go listener lifecycle, error handling, and shutdown behavior
- verify multi-tunnel behavior from YAML `run`
- compare against captured Go tunnel traces in `compat/golden/runtime/go_test_pkg_tun.txt`

## SOCKS Proxy Work

- implement local SOCKS proxy runtime
- match Go support for SOCKS4
- match Go support for SOCKS5
- match feature limitations and unsupported modes exactly
- verify local listen defaults and error paths
- integrate with SSH transport and reconnect behavior

## DNS Proxy Work

- implement local UDP listener
- implement local TCP listener
- implement DNS-over-TCP framing through SSH
- implement `--remote-dns-server` default and config-driven remote DNS address behavior
- verify behavior under `run`
- compare against Go runtime and fixtures
- decide whether to preserve the apparent `run` DNS-client selection bug if confirmed

## SFTP Work

- implement SFTP client download path for `get`
- implement SFTP client upload path for `put`
- implement resumable transfers
- implement chunked concurrent transfers
- implement worker-pool behavior equivalent to Go `pkg/worker`
- implement recursive transfer logic
- implement embedded server SFTP subsystem
- implement subsystem-disable behavior
- compare transfer concurrency defaults and limits exactly

## `run` Command Work

- replace the current placeholder `nothing to run` hardcoded timestamp output with verified Go-equivalent behavior
- orchestrate all configured sections:
  - `sshclient`
  - `tunnel`
  - `sshd`
  - `socksproxy`
  - `dnsproxy`
- ensure config composition behavior matches Go when multiple sections are present
- preserve Go startup ordering if it is user-visible
- preserve Go error propagation and exit behavior if one subsystem fails while others start
- preserve quiet/logging behavior across all spawned components

## Logging Work

- replace placeholder `init_logging`
- match Go logger formatting and prefixes
- match color behavior
- match quiet-mode suppression
- match command/runtime log wording where user-visible and compatibility-sensitive
- verify whether logs go to stdout vs stderr on each path

## Cross-Platform Work

- verify Linux runtime behavior
- verify macOS runtime behavior
- implement Windows service mode equivalent to Go `go-svc`
- implement Windows PTY support equivalent to ConPTY path in Go
- verify Windows banner suppression and shell behavior
- verify path expansion and home-directory behavior on Windows
- verify file-permission semantics for key files on Windows

## Test Suite Work

- extend CLI golden coverage beyond help/template/root to real runtime outputs
- add Rust-vs-Go side-by-side integration tests
- boot Go and Rust implementations against each other in automated tests
- add failure-path coverage for:
  - unknown hosts
  - invalid keys
  - auth failure
  - tunnel listener collisions
  - network interruption
  - reconnect recovery
- turn the currently manual live interoperability checks into automated tests
- add exact exit-code assertions for all commands
- add more config compatibility fixtures
- add end-to-end tests for `run`

## Interop Validation Work

- validate Go client -> Rust server
- validate Rust client -> Go server for all commands, not just `grabpubkey` and keyed `shell`
- validate Rust server -> Go client behavior
- validate forward tunnels across mixed Go/Rust combinations
- validate reverse tunnels across mixed Go/Rust combinations
- validate SOCKS across mixed Go/Rust combinations
- validate SFTP across mixed Go/Rust combinations
- validate known-hosts enrollment and trust behavior across mixed binaries

## Documentation Work

- keep `docs/migration/report.md` in sync with the actual implementation state or retire it in favor of the new top-level docs
- document any proven unavoidable differences only after they are verified
- add a final migration report once runtime parity is real

## Current Known Mismatches And Incomplete Behaviors

- `shell --jump-host` is accepted but ignored
- `run` only parses config and returns placeholder behavior
- non-help runtime commands other than `keygen`, `grabpubkey`, and `shell` still return `Rust runtime implementation is not complete yet`
- logging parity is absent
- reconnect/keepalive behavior is absent
- no embedded server exists yet
- no tunnel engine exists yet
- no SOCKS runtime exists yet
- no DNS proxy runtime exists yet
- no SFTP runtime exists yet
- no Windows service mode exists yet
- no cross-platform parity validation exists yet
- one no-auth live SSH interop path is currently broken

## Proven-Complete Areas So Far

- Go CLI and config surface have been inventoried
- Go golden fixtures are reproducible
- Rust YAML schema mirror exists
- Rust utility behavior is partially fixture-matched
- Rust `keygen` exists
- Rust `grabpubkey` exists
- Rust `shell` can execute a command successfully against a keyed Go server
