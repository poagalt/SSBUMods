#!/usr/bin/env bash
# The community's reference data, cloned rather than copied, so it can be refreshed.
#
#   param-labels               names for every field in every .prc -- the frame data and speeds
#                              this moveset reads are meaningless numbers without it
#   SSBU-Dumped-Motion-Lists   every vanilla motion_list, where FAF and the motion flags live
#   motion_lib / prc-rs        the Rust readers for those two formats
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
mkdir -p vendor/refs
for r in ultimate-research/param-labels WuBoytH/SSBU-Dumped-Motion-Lists \
         ultimate-research/motion_lib ultimate-research/prc-rs; do
    n=$(basename "$r")
    if [ -d "vendor/refs/$n/.git" ]; then
        echo "  updating $n"; git -C "vendor/refs/$n" pull --ff-only >/dev/null 2>&1 || true
    else
        echo "  cloning $n"; git clone --depth 1 "https://github.com/$r" "vendor/refs/$n" >/dev/null 2>&1
    fi
done
du -sh vendor/refs
