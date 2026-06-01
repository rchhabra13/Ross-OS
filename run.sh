#!/usr/bin/env bash
# Build the kernel and boot it in QEMU (virt machine, serial to stdout).
# Auto-kills after $1 seconds (default 5) since the kernel loops forever.
set -euo pipefail
cd "$(dirname "$0")"

SECS="${1:-5}"
KERNEL="target/aarch64-unknown-none/debug/kernel"

cargo build

echo "=== QEMU serial output (auto-stop ${SECS}s) ==="
qemu-system-aarch64 -M virt -cpu cortex-a72 -m 1G -nographic -kernel "$KERNEL" &
QPID=$!
sleep "$SECS"
kill "$QPID" 2>/dev/null || true
wait "$QPID" 2>/dev/null || true
