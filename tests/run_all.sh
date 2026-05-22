#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/localhost"
CONF="$ROOT/configs/default.conf"

cargo build --release --manifest-path "$ROOT/Cargo.toml"
"$BIN" "$CONF" &
PID=$!
trap 'kill $PID 2>/dev/null || true' EXIT
sleep 0.4

curl -sf http://127.0.0.1:8080/ >/dev/null
curl -sf -o /dev/null -w '%{http_code}' http://127.0.0.1:8080/nope | grep -q 404
curl -sf -o /dev/null -w '%{http_code}' -X DELETE http://127.0.0.1:8080/assets/style.css | grep -qE '405|404'
code=$(curl -s -o /dev/null -w '%{http_code}' -L http://127.0.0.1:8080/legacy)
test "$code" = "200" -o "$code" = "301" -o "$code" = "302"
curl -sf http://127.0.0.1:8080/cgi/hello.py | grep -q Python
echo "All smoke tests passed."
