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
- upstream `russh` usage without a local patched dependency override
- YAML 1.1-style boolean compatibility for config fields such as `forward: yes`
- targeted maintainability comments in dense runtime, PTY, progress, config, and CLI-precedence code paths
- SSH client trust-error handling for unknown hosts and malformed `known_hosts`
- SSH client password-prompt fallback, no-auth coverage, disconnect-aware keepalive handling, and shell stdout/stderr/exit propagation coverage

## Highest Priority Remaining Work

- validate Windows support end to end:
  - Windows service mode behavior
  - Windows PTY/ConPTY behavior
  - Windows-specific path, permission, and banner semantics
- finish exhaustive exit-code parity beyond the currently covered representative cases
- extend side-by-side behavioral diff coverage beyond the currently covered representative commands
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
- verify additional YAML scalar edge cases beyond the now-supported boolean spellings

## SSH Client Work

- verify stock keepalive behavior remains stable across longer-lived runtime sessions
- verify password-auth prompt behavior against more real OpenSSH client/server combinations
- verify stdout/stderr interleaving in more edge cases beyond the currently covered shell command path
- confirm whether encrypted/passphrase identity files matter at all, since the Go client does not currently support them either

## Embedded SSH Server Work

- verify the implemented Windows ConPTY behavior against the Go server path
- verify shell/session behavior against Go in more OpenSSH client combinations
- verify forwarding lifecycle and teardown behavior against Go more exhaustively
- verify disable-auth behavior across mixed Go/Rust client-server combinations
- expose or test active-session-count parity if that contract matters

## Tunnel Engine Work

- compare listener lifecycle, reconnect timing, and shutdown semantics side by side with Go
- keep the 5-second tunnel keepalive cadence covered by tests

## SOCKS Proxy Work

- verify unsupported SOCKS modes and failure replies match Go exactly
- verify listen-default and bind-error wording/exit codes

## DNS Proxy Work

- verify exact UDP/TCP failure semantics against Go
- validate `run` orchestration behavior and failure handling for DNS proxy sections
- decide whether to preserve the apparent Go DNS-client selection bug in `run`

## SFTP Work

- verify exact mpb-style progress rendering and line formatting against Go on real terminals
- verify remaining recursive transfer edge cases against Go
- validate permission behavior across more Rust-side combinations

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
- extend `behavioral_diff.rs` to more binary commands and more failure cases
- extend `run` coverage with realistic mixed configs
- decide what to do about the Go test packages that do not have direct Rust module equivalents:
  - `pkg/registry`
  - `pkg/worker`
  - `pkg/rio`

## Optional Reference Validation

- keep using Go behavior and fixtures as reference points where useful for regressions
- diff Go and Rust outputs/behaviors side by side where user-visible if a parity question comes up

## Documentation Work

- keep `ARCHITECTURE.md`, `DECISIONS.md`, and `SPEC.md` in sync with implementation changes
- refresh `docs/migration/report.md` or retire it if the top-level docs are the new source of truth
- document any unavoidable differences only after they are verified

## Current Known Gaps

- Windows support is implemented but not yet validated end to end on a real Windows host
- exact Go mpb-style interactive SFTP progress parity is not proven
- full exit-code/error-text parity is not proven beyond the currently covered representative cases
- side-by-side behavioral diff coverage is representative, not exhaustive
- some Go test coverage has no direct Rust module equivalent yet
