# Rust Migration Report

Date: 2026-03-19

## Goal

Produce a Rust `rospo` binary that is a drop-in replacement for the Go version, preserving CLI surface, YAML compatibility, SSH protocol behavior, tunnel semantics, logging, and exit-code behavior.

## Work Completed In This Iteration

### Phase 1: Go analysis

- Mapped the full Cobra command tree and all shared/common flags.
- Mapped the YAML schema used by `pkg/conf`, `pkg/sshc`, `pkg/sshd`, and `pkg/tun`.
- Documented module boundaries and runtime invariants in `docs/migration/go_inventory.md`.
- Identified compatibility-sensitive behaviors:
  - SSH config expansion from `~/.ssh/config`
  - host-key enrollment and `known_hosts` mutation
  - 5-second reconnect and keepalive loops
  - custom reverse-forward requests (`tcpip-forward`, `forwarded-tcpip`, `checkalive@rospo`, `keepalive@rospo`)
  - banner behavior and Windows service/PTTY differences

### Phase 2: Golden baselines

- Added `scripts/capture_go_baselines.sh`.
- Added `tools/go_baseline` to serialize Go parsing behavior into JSON fixtures.
- Captured CLI help and root invocation outputs under `compat/golden/cli`.
- Captured config and parser outputs under `compat/golden/runtime`.
- Captured verbose networked package test traces under:
  - `compat/golden/runtime/go_test_pkg_sshc.txt`
  - `compat/golden/runtime/go_test_pkg_sshd.txt`
  - `compat/golden/runtime/go_test_pkg_tun.txt`

### Phase 3: Rust project creation

- Added a Rust crate in `rust/`.
- Declared the requested dependency set:
  - `clap`
  - `serde`
  - `serde_yaml`
  - `tokio`
  - `russh`
  - `tracing`
- Added module skeletons mirroring the Go project areas:
  - `cli`
  - `config`
  - `logging`
  - `ssh`
  - `tunnel`
  - `socks`
  - `sftp`

### Early compatibility tests

- Added Rust tests for the existing Go config fixtures.
- Verified:
  - Rust crate builds
  - Rust config tests pass
  - Go networked tests pass when executed outside the sandbox restrictions

## Current State

The repo now contains:

- a documented contract for the Go implementation
- reproducible baseline fixtures from the Go binary and Go runtime packages
- a buildable Rust crate with the correct high-level module layout
- initial Rust config compatibility tests

The repo does **not** yet contain a full Rust runtime implementation for:

- exact Cobra-compatible help rendering
- SSH client/server behavior via `russh`
- forward/reverse tunnel engine
- SOCKS4/5 proxy
- SFTP client/server support
- Windows service mode
- side-by-side Go/Rust behavioral diff tests

## Important Findings

- Root no-arg invocation exits with code `0` while printing an error and usage text.
- Unknown-host behavior is split:
  - `grabpubkey` adds keys to `known_hosts`
  - regular SSH connections fail hard until the key is trusted
- Reverse tunneling depends on SSH remote forwarding support and a server-side keepalive probe.
- The Go implementation appears to contain a bug in `cmd/run.go` for DNS proxy dedicated SSH config selection; the Rust port must decide whether to preserve that bug for strict compatibility or fix it and document the difference.

## Unavoidable Differences

None identified yet at the protocol or schema level.

This section is intentionally conservative and should remain empty until a verified incompatibility is proven.

## Next Work Items

1. Replace the placeholder Rust CLI execution path with a fixture-driven command renderer that matches the Go help text and exit codes.
2. Port SSH URL parsing and SSH config expansion logic from `pkg/utils`.
3. Implement the SSH client state machine and known-host behavior in Rust.
4. Port the embedded SSH server and reverse-forward request handling.
5. Port tunnel, SOCKS, DNS, and SFTP layers using the captured Go traces as regression oracles.
6. Add side-by-side integration tests that boot Go and Rust implementations against each other.
