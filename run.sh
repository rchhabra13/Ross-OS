#!/usr/bin/env bash
# Build the kernel and boot it in QEMU (virt machine + ramfb framebuffer).
#
#   ./run.sh               headless, serial to stdout, auto-stops after 5s
#   ./run.sh 8             headless, auto-stops after 8s
#   ./run.sh gui           open a graphical window (close it to quit)
#   ./run.sh shot out.ppm  headless, dump the screen to a PPM then quit
set -euo pipefail
cd "$(dirname "$0")"

KERNEL="target/aarch64-unknown-none/debug/kernel"
cargo build

COMMON=(-M virt -cpu cortex-a72 -m 1G -device ramfb -kernel "$KERNEL")

case "${1:-}" in
  gui)
    echo "=== QEMU GUI (close window to quit) ==="
    exec qemu-system-aarch64 "${COMMON[@]}" -serial stdio -display cocoa
    ;;
  shot)
    OUT="${2:-screen.ppm}"
    echo "=== capturing screen to ${OUT} ==="
    {
      sleep 3
      printf 'screendump %s\n' "$OUT"
      sleep 1
      printf 'quit\n'
    } | qemu-system-aarch64 "${COMMON[@]}" -serial null -display none -monitor stdio
    ;;
  *)
    SECS="${1:-5}"
    echo "=== QEMU serial output (auto-stop ${SECS}s) ==="
    qemu-system-aarch64 "${COMMON[@]}" -serial stdio -display none &
    QPID=$!
    sleep "$SECS"
    kill "$QPID" 2>/dev/null || true
    wait "$QPID" 2>/dev/null || true
    ;;
esac
