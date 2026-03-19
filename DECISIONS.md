# Rospo Migration Decisions

Date: 2026-03-19

This document records the important technical decisions made so far during the migration. These are decisions already reflected in the repository, not speculative future choices.

## 1. Keep The Go Codebase As The Compatibility Oracle

Decision:

- the Go implementation remains in the repository and is treated as the ground truth for behavior

Why:

- the migration target is behavioral equivalence, not a redesign
- exact CLI, config, and runtime behavior cannot be trusted to memory or manual reimplementation

Consequences:

- Go code is still used to generate baselines
- compatibility work is measured against Go outputs and Go runtime traces
- the Rust port is intentionally built alongside, not in place of, the Go code

## 2. Capture Golden Fixtures From The Real Go Binary

Decision:

- CLI behavior is captured from the built Go binary into golden fixtures under `compat/golden/cli`

Why:

- Cobra output formatting is compatibility-sensitive
- command help wording, ordering, spacing, and exit codes are easy to get subtly wrong

Consequences:

- root help, root no-arg output, command help, and template output are all fixture-backed
- Rust tests compare outputs directly against those fixtures
- the baseline capture flow is reproducible through `scripts/capture_go_baselines.sh`

## 3. Capture Structured Runtime Baselines With A Small Go Helper

Decision:

- structured parser/config behavior is captured with `tools/go_baseline`

Why:

- parser behavior is easier to compare through JSON than through raw logs
- reusing the real Go implementation removes guesswork around defaults and edge cases

Consequences:

- config parsing, SSH URL parsing, and SSH config parsing now have JSON baselines
- Rust tests compare against those baselines
- a small adapter was added in `pkg/utils/ssh_config_parser_baseline.go` rather than duplicating Go parser logic

## 4. Prioritize Exact CLI Output Matching Before Full Runtime Porting

Decision:

- the early Rust CLI path focuses on exact fixture matching for help/template/root behavior before implementing every command runtime

Why:

- CLI parity is user-visible and easy to regress
- it is cheaper to lock the command surface first and then fill in runtime behavior

Consequences:

- help outputs are currently served from Go-captured fixtures
- several commands are still runtime placeholders even though their help is already matched

## 5. Use Manual CLI Dispatch For Now

Decision:

- the current Rust CLI uses manual argument parsing and dispatch in `rust/src/cli/mod.rs`

Why:

- exact Cobra-compatible output and behavior were faster to guarantee manually at this stage
- `clap` is part of the intended stack, but direct use was deferred while compatibility details were still being pinned down

Consequences:

- `clap` is declared in `Cargo.toml` but is not yet driving live parsing
- shared flag parsing is currently handwritten
- a future decision is still needed on whether to keep manual parsing or introduce `clap` without losing compatibility

## 6. Keep The Rust Port In A Separate `rust/` Crate For Now

Decision:

- the Rust implementation lives under `rust/` rather than replacing the top-level project layout

Why:

- both implementations need to coexist during migration
- the Go project must remain runnable to regenerate fixtures and validate parity

Consequences:

- the repository currently contains both a Go binary path and a Rust binary path
- migration work can progress incrementally without deleting the oracle implementation

## 7. Mirror The Go YAML Schema Directly With `serde` Renames

Decision:

- Rust config structs preserve the Go YAML field names directly with `serde(rename = "...")`

Why:

- config compatibility is mandatory
- this is the simplest path to preserving key names and top-level section names exactly

Consequences:

- `rust/src/config/mod.rs` mirrors the Go schema closely
- defaults are represented using `Option` and `#[serde(default)]` to match Go decoding behavior as closely as currently understood

## 8. Do Not Add A Rust-Side Post-Load Config Defaulting Layer Yet

Decision:

- the current Rust config loader is a direct `serde_yaml::from_str`

Why:

- Go behavior shows that defaults mostly live in CLI code and runtime constructors, not a central config-normalization pass
- adding an eager defaulting layer too early would risk drifting from Go

Consequences:

- runtime code must continue to apply defaults where Go does
- config tests focus on raw decode semantics first

## 9. Port Utility Behavior Before Runtime Composition

Decision:

- SSH URL parsing, SSH config parsing, home expansion, and known-hosts formatting were ported ahead of tunnels and services

Why:

- these utilities are shared across multiple runtime features
- they are easier to baseline and verify independently

Consequences:

- `rust/src/utils/mod.rs` contains real compatibility-sensitive behavior already covered by tests
- higher-level runtime code can build on those helpers later

## 10. Implement Real SSH Protocol Paths Early With `russh`

Decision:

- `grabpubkey` and `shell` were implemented using real SSH sessions through `russh`

Why:

- protocol interoperability is a hard requirement
- early real interop catches issues that fixture-only testing cannot

Consequences:

- Rust already performs live handshakes against Go `sshd`
- known-hosts enrollment uses actual server keys from a real handshake
- command execution has been proven to work against a keyed Go server

## 11. Preserve Go-Compatible Known-Hosts Line Formatting

Decision:

- Rust appends known-hosts entries in the same host formatting style observed from Go

Why:

- host-key trust files must interoperate across Go and Rust binaries

Consequences:

- default-port IPv4/hostname entries are written as `host key`
- non-default ports and bracketed hosts are written as `[host]:port key`
- Rust tests check this formatting explicitly

## 12. Match Go `keygen` Output Format, Not Just Key Type

Decision:

- Rust `keygen` emits:
  - P-521 private keys as SEC1 PEM with `BEGIN EC PRIVATE KEY`
  - public keys as OpenSSH `ecdsa-sha2-nistp521`

Why:

- drop-in replacement requires file and stdout compatibility, not just cryptographic equivalence

Consequences:

- `p521`, `sec1`, and `internal-russh-forked-ssh-key` are used together for output compatibility
- stored private and public keys are written separately

## 13. Preserve Unix `0600` Semantics For Generated Sensitive Files

Decision:

- Rust uses a helper to write generated key material and known-hosts files with `0600` on Unix

Why:

- Go writes these files with restrictive permissions
- permissive permissions would be a behavioral regression and could affect SSH tooling

Consequences:

- `write_file_0600` is used by current key and known-hosts paths

## 14. Use Tokio Runtime Creation Inside Command Handlers For Current Implemented Commands

Decision:

- `grabpubkey` and `shell` create a current-thread Tokio runtime inside the command handler

Why:

- the existing CLI entrypoint is synchronous
- this allowed real async SSH operations without redesigning the whole command architecture yet

Consequences:

- the current implementation is simple but not yet the final runtime composition model
- future multi-service commands like `run` will likely require a broader runtime architecture

## 15. Record SSH Protocol Constants Even Before Full Keepalive Support

Decision:

- the Rust SSH module already defines `keepalive@rospo` and `checkalive@rospo` constants

Why:

- these are known protocol-level compatibility points from the Go implementation
- they should be explicit in the codebase even before the full feature is wired up

Consequences:

- the constants exist today
- the behavior using them is not fully implemented yet

## 16. Accept Partial Progress Only When It Is Tested

Decision:

- each implemented slice has been paired with Rust tests and, where possible, live Go/Rust interop checks

Why:

- the migration requirement is behavioral equivalence
- untested ports are not trustworthy

Consequences:

- there are automated tests for CLI, config, and utility behavior
- there are documented live checks for `grabpubkey` and `shell`
- incomplete features remain explicitly marked incomplete instead of being treated as done

## 17. Do Not Declare Unavoidable Differences Without Proof

Decision:

- no unavoidable differences have been declared yet

Why:

- the migration goal is strict equivalence
- declaring differences early would lower the compatibility bar without evidence

Consequences:

- current docs intentionally keep the unavoidable-differences list empty
- any future differences must be proven, documented, and justified

## 18. Keep A Suspected Go Bug Visible Instead Of Silently Fixing It

Decision:

- the apparent Go `run` DNS proxy SSH-client-selection bug has been documented but not “corrected” in Rust

Why:

- strict compatibility may require preserving Go bugs
- changing behavior before validation would risk incompatibility

Consequences:

- the bug is tracked in the spec and todo documents
- a later explicit decision is still required after validating the Go path

## 19. Favor Compatibility Over Idiomatic Rust

Decision:

- implementation choices so far optimize for matching the Go binary rather than producing the most idiomatic Rust architecture

Why:

- this is the user’s stated requirement
- correctness against the existing binary matters more than architectural elegance

Consequences:

- fixture-backed output paths are used directly
- manual parsing is used where it currently improves fidelity
- module placeholders exist so the Rust layout can grow around compatibility needs rather than around aesthetics
