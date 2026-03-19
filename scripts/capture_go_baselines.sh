#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_PATH="/tmp/rospo-go-baseline"
OUT_DIR="$ROOT_DIR/compat/golden"
GOCACHE_DIR="/tmp/rospo-gocache"

mkdir -p "$OUT_DIR/cli" "$OUT_DIR/runtime" "$GOCACHE_DIR"

export GOCACHE="$GOCACHE_DIR"

pushd "$ROOT_DIR" >/dev/null
go build -o "$BIN_PATH" .

capture_cmd() {
  local name="$1"
  shift
  local output_file="$OUT_DIR/cli/${name}.txt"
  local exit_file="$OUT_DIR/cli/${name}.exitcode"

  set +e
  "$BIN_PATH" "$@" >"$output_file" 2>&1
  local rc=$?
  set -e
  printf '%s\n' "$rc" >"$exit_file"
}

capture_cmd root-help --help
capture_cmd root-noargs
capture_cmd dns-proxy-help dns-proxy --help
capture_cmd get-help get --help
capture_cmd grabpubkey-help grabpubkey --help
capture_cmd keygen-help keygen --help
capture_cmd put-help put --help
capture_cmd revshell-help revshell --help
capture_cmd run-help run --help
capture_cmd shell-help shell --help
capture_cmd socks-proxy-help socks-proxy --help
capture_cmd sshd-help sshd --help
capture_cmd template-help template --help
capture_cmd tun-help tun --help
capture_cmd tun-forward-help tun forward --help
capture_cmd tun-reverse-help tun reverse --help
capture_cmd template-output template

go run ./tools/go_baseline config ./pkg/conf/testdata/sshc.yaml >"$OUT_DIR/runtime/config_sshc.json"
go run ./tools/go_baseline config ./pkg/conf/testdata/sshc_insecure.yaml >"$OUT_DIR/runtime/config_sshc_insecure.json"
go run ./tools/go_baseline config ./pkg/conf/testdata/sshc_secure_default.yaml >"$OUT_DIR/runtime/config_sshc_secure_default.json"
go run ./tools/go_baseline config ./pkg/conf/testdata/sshd.yaml >"$OUT_DIR/runtime/config_sshd.json"
go run ./tools/go_baseline ssh-url user@192.168.0.1:22 >"$OUT_DIR/runtime/ssh_url_ipv4.json"
go run ./tools/go_baseline ssh-url :22 >"$OUT_DIR/runtime/ssh_url_empty_host.json"
go run ./tools/go_baseline ssh-url user@[2001:0db8:85a3:0000:0000:8a2e:0370:7334]:2222 >"$OUT_DIR/runtime/ssh_url_ipv6.json"
go run ./tools/go_baseline ssh-config ./pkg/utils/testdata/ssh_config >"$OUT_DIR/runtime/ssh_config.json"

popd >/dev/null
