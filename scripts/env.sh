#!/usr/bin/env bash
# mdreview build environment.
# - conda rustbuild provides a host C compiler/sysroot for build scripts and proc macros
# - the final target is static x86_64-unknown-linux-musl linked with Rust's bundled rust-lld
set -euo pipefail
source "$HOME/anaconda3/etc/profile.d/conda.sh"
conda activate rustbuild
export PATH="$HOME/.cargo/bin:$PATH"
export CC="${CC:-x86_64-conda-linux-gnu-cc}"
export CXX="${CXX:-x86_64-conda-linux-gnu-c++}"
export LD="${LD:-x86_64-conda-linux-gnu-ld}"
export AR="${AR:-x86_64-conda-linux-gnu-ar}"
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="${CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER:-x86_64-conda-linux-gnu-cc}"
