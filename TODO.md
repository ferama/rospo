# Rospo Migration TODO

Date: 2026-03-19

This file lists the remaining work after the current Rust implementation state. It intentionally excludes work that is already implemented and validated.

## Completion Status

The migration is still not complete. The Rust binary is not yet a proven drop-in replacement.

Already implemented in Rust:

- fixture-matched root help and root no-arg output
- fixture-matched help output for captured commands
- fixture-matched template output
- config schema mirror and config file loading
- SSH URL parsing, endpoint formatting, SSH config parsing, known-hosts formatting
- `keygen`
- `grabpubkey`
- `shell`
- `socks-proxy`
- `dns-proxy`
- `get`
- `put`
- `tun forward`
- `tun reverse`
- `sshd`
- `revshell`
- non-placeholder `run` orchestration for implemented sections
- HTTP/HTTPS `authorized_keys` support
- Unix PTY-backed embedded-server shell handling
- Rust automated coverage for config, utils, keys, SSH, SSHD, SOCKS, and tunnels

## Highest Priority Remaining Work

- finish exact logging/output parity with the Go binary
- finish Windows support:
  - Windows service mode
  - Windows PTY/ConPTY support
- finish Go-equivalent concurrent/chunked SFTP transfer behavior
- finish exhaustive exit-code and malformed-invocation parity
- automate mixed Go/Rust interoperability validation across the full matrix

## CLI Parity Work

- verify exact exit codes for all success and failure paths, not just the currently covered ones
- verify unknown-flag and malformed-invocation behavior against Cobra for every command
- verify `help` subcommand parity beyond the currently captured combinations
- decide whether the current manual parsing approach is sufficient long term or if a lower-risk parser abstraction is needed
- wire root `-q/--quiet` behavior consistently through all commands and runtime paths

## Config Layer Work

- add Rust tests for more Go-style config compositions:
  - tunnel-oriented configs
  - socks proxy configs
  - DNS proxy configs
  - mixed multi-section configs used by `run`
- verify unknown-field ignoring matches Go in more mixed config scenarios
- verify runtime defaulting behavior for every config-backed command path
- decide whether the apparent Go `run` DNS-client selection bug must be preserved exactly

## SSH Client Work

- verify exact keepalive behavior versus Go:
  - timing
  - request type
  - disconnect handling
- verify host-key failure wording and exit codes match Go
- verify password-auth behavior and prompts against Go CLI/OpenSSH expectations
- verify no-auth server behavior against Go across more combinations
- verify stdout/stderr interleaving and exit-status propagation in more edge cases
- verify identity-file loading behavior including encrypted/passphrase cases if Go supports them
- verify malformed known-hosts behavior matches Go

## Embedded SSH Server Work

- implement Windows PTY behavior equivalent to the Go ConPTY path
- verify shell/session behavior against Go in more OpenSSH client combinations
- verify forwarding lifecycle and teardown behavior against Go more exhaustively
- verify disable-auth behavior across mixed Go/Rust client-server combinations
- expose or test active-session-count parity if that contract matters

## Tunnel Engine Work

- verify Go-equivalent `checkalive@rospo` behavior and timing
- compare listener lifecycle, reconnect timing, and shutdown semantics side by side with Go
- add automated mixed-version tunnel tests:
  - Go client -> Rust server
  - Rust client -> Go server
  - Go server -> Rust client

## SOCKS Proxy Work

- verify unsupported SOCKS modes and failure replies match Go exactly
- validate mixed Go/Rust SOCKS behavior in both directions
- verify listen-default and bind-error wording/exit codes

## DNS Proxy Work

- verify exact UDP/TCP failure semantics against Go
- validate `run` orchestration behavior and failure handling for DNS proxy sections
- decide whether to preserve the apparent Go DNS-client selection bug in `run`

## SFTP Work

- port Go-equivalent chunked concurrent uploads
- port Go-equivalent chunked concurrent downloads
- port or emulate Go worker-pool behavior and limits
- verify resume semantics match Go
- verify recursive transfer edge cases against Go
- add automated mixed Go/Rust SFTP coverage beyond the current manual/live validations

## `run` Command Work

- verify exact startup ordering against Go
- verify exact failure propagation if one subsystem fails while others start
- verify shutdown behavior on `ctrl-c`
- verify logging/quiet behavior across all spawned subsystems
- add end-to-end tests for realistic multi-section configs

## Logging Work

- replace the current placeholder `rust/src/logging/mod.rs`
- match Go logger prefixes, colors, wording, and formatting
- match quiet-mode suppression behavior
- verify stdout versus stderr placement on each path

## Cross-Platform Work

- verify Linux runtime behavior end to end
- verify macOS runtime behavior end to end
- implement Windows service mode equivalent to Go `go-svc`
- implement Windows ConPTY-backed shell/session handling
- verify Windows banner suppression and shell behavior
- verify Windows home/path expansion semantics
- verify Windows file-permission semantics for key files

## Test Suite Work

- add exact exit-code assertions for more command/runtime paths
- add failure-path coverage for:
  - unknown hosts
  - invalid keys
  - auth failure
  - listener collisions
  - network interruption
  - reconnect recovery
- turn the current manual live interop checks into automated tests where practical
- extend `run` coverage with realistic mixed configs
- decide what to do about the Go test packages that do not have direct Rust module equivalents:
  - `pkg/registry`
  - `pkg/worker`
  - `pkg/rio`

## Interop Validation Work

- validate the full mixed-version matrix for all implemented commands
- diff Go and Rust outputs/behaviors side by side where user-visible
- validate known-hosts enrollment/trust behavior across mixed binaries
- validate SFTP, SOCKS, DNS, and tunnels across mixed Go/Rust combinations systematically

## Documentation Work

- keep `ARCHITECTURE.md`, `DECISIONS.md`, and `SPEC.md` in sync with implementation changes
- refresh `docs/migration/report.md` or retire it if the top-level docs are the new source of truth
- document any unavoidable differences only after they are verified

## Current Known Gaps

- logging parity is not done
- Windows support is not done
- chunked concurrent SFTP parity is not done
- full exit-code/error-text parity is not proven
- full mixed Go/Rust automated interoperability coverage is not done
- some Go test coverage has no direct Rust module equivalent yet
