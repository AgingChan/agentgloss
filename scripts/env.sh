#!/usr/bin/env bash
# mdreview build environment. Source this file; do not execute it.
# - conda rustbuild provides a host C compiler/sysroot for build scripts and proc macros
# - the final target is static x86_64-unknown-linux-musl linked with Rust's bundled rust-lld
#
# This file deliberately does not set `set -euo pipefail`: it is sourced into
# interactive shells, and leaving `nounset` enabled breaks prompt hooks such as
# Terax's `[[ -n "$TERAX_BLOCKS" ]]`.

if [ ! -r "$HOME/anaconda3/etc/profile.d/conda.sh" ]; then
    echo "ERROR: cannot find $HOME/anaconda3/etc/profile.d/conda.sh" >&2
    echo "       install conda or adjust scripts/env.sh for this machine." >&2
    return 1 2>/dev/null || exit 1
fi

# shellcheck source=/dev/null
. "$HOME/anaconda3/etc/profile.d/conda.sh"
if ! conda activate rustbuild; then
    echo "ERROR: conda environment 'rustbuild' is not available." >&2
    echo "       create it or adjust scripts/env.sh for this machine." >&2
    return 1 2>/dev/null || exit 1
fi

export PATH="$HOME/.cargo/bin:$PATH"
export CC="${CC:-x86_64-conda-linux-gnu-cc}"
export CXX="${CXX:-x86_64-conda-linux-gnu-c++}"
export LD="${LD:-x86_64-conda-linux-gnu-ld}"
export AR="${AR:-x86_64-conda-linux-gnu-ar}"
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="${CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER:-x86_64-conda-linux-gnu-cc}"
