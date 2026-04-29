#!/usr/bin/env bash
set -euo pipefail

AR_x86_64_unknown_linux_gnu="${AR_x86_64_unknown_linux_gnu:-/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-ar}" \
  cargo zigbuild --release --target x86_64-unknown-linux-gnu
