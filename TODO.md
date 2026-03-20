# Rospo Migration TODO

Date: 2026-03-20

This file lists the remaining work after the current Rust implementation state. It intentionally excludes work that is already implemented and validated.

## Completion Status

The migration is still not complete. The Rust binary is not yet a proven drop-in replacement.

Already implemented in Rust:

- clap-driven root help and root no-arg output
- clap-driven help output for implemented commands
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
- Windows service entrypoint and SCM detection
- Windows ConPTY-backed embedded-server shell handling
- Unix client-side raw-terminal handling and immediate interactive shell teardown
- root `-q/--quiet` acceptance and global quiet-mode suppression
- `clap` derive-based CLI parsing, typed subcommand dispatch, and direct `Cli::parse()` entrypoint
- Go-style stdout logger with timestamps, prefixes, ANSI colors, and quiet suppression
- chunked concurrent single-file SFTP upload/download
- bounded concurrent recursive SFTP transfer scheduling
- Go-style per-chunk retry worker behavior for SFTP transfers
- resumed SFTP progress accounting
- recursive SFTP `get` root-directory preservation
- transferred-file permission preservation for SFTP upload/download
- maintainability refactor from large monolithic `mod.rs` files into package-style focused submodules for `cli`, `sshd`, `ssh`, `sftp`, and `utils`
- Rust automated coverage for config, utils, keys, SSH, SSHD, SOCKS, tunnels, chunked SFTP, malformed CLI parity, Rust->Go interop, and side-by-side Go/Rust behavioral diffing for representative binary/runtime paths

## Highest Priority Remaining Work

- validate Windows support end to end:
  - Windows service mode behavior
  - Windows PTY/ConPTY behavior
  - Windows-specific path, permission, and banner semantics
- finish exhaustive exit-code parity beyond the currently covered representative cases
- automate mixed Go/Rust interoperability validation across the full matrix
- extend side-by-side Go/Rust binary diff coverage beyond the currently covered representative commands
- finish exact Go mpb/progress-output parity for interactive terminal SFTP rendering

## CLI Parity Work

- verify exact exit codes for all success and failure paths, not just the currently covered ones
- extend the current malformed-invocation regression coverage to all commands and more edge cases
- verify clap-generated help remains complete and accurate for all commands and nested subcommands
- keep clap arg definitions and runtime option conversion logic in sync as features change

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

- verify the implemented Windows ConPTY behavior against the Go server path
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

- verify exact mpb-style progress rendering and line formatting against Go on real terminals
- verify remaining recursive transfer edge cases against Go
- validate permission behavior against mixed Go/Rust server-client combinations
- extend automated mixed Go/Rust SFTP coverage beyond the current Rust-client-to-Go-server cases

## `run` Command Work

- verify exact startup ordering against Go
- verify exact failure propagation if one subsystem fails while others start
- verify shutdown behavior on `ctrl-c`
- verify logging/quiet behavior across all spawned subsystems
- add end-to-end tests for realistic multi-section configs

## Logging And Output Work

- extend byte-for-byte side-by-side output diff coverage beyond the current `grabpubkey`, `shell`, `get`, and `put` cases
- verify stdout versus stderr placement on each path
- verify remaining failure-path wording and formatting against Go

## Cross-Platform Work

- verify Linux runtime behavior end to end
- verify macOS runtime behavior end to end
- verify Windows service mode equivalent to Go `go-svc`
- verify Windows ConPTY-backed shell/session handling
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
- extend automated Go/Rust interop tests beyond the current shell/SFTP/tunnel coverage
- extend `behavioral_diff.rs` to more binary commands and more failure cases
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

- Windows support is implemented but not yet validated end to end on a real Windows host
- exact Go mpb-style interactive SFTP progress parity is not proven
- full exit-code/error-text parity is not proven beyond the currently covered representative cases
- full mixed Go/Rust automated interoperability coverage is not done
- side-by-side binary diff coverage is representative, not exhaustive
- some Go test coverage has no direct Rust module equivalent yet
