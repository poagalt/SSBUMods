#!/usr/bin/env bash
# Runs nxdumptool's PC-side host, which receives a dump over USB straight to this machine.
#
# WHY WE WANT THIS AT ALL: see tools/arc-pull/src/bin/arc-bulk.rs. Extraction over arc_network is
# the one operation that needs Smash running, and it is latency-bound. A local copy of
# rom:/data.arc removes the console from the loop entirely and makes the WHOLE archive available
# rather than whichever subset was filtered for on the day.
#
# SETUP THAT IS ALREADY DONE (nothing to repeat):
#   .venv                with pyusb, tqdm and libusb-package
#   libusb-1.0.dll       copied out of libusb-package next to the script, because PyUSB looks for
#                        it by name and does not bundle one
#   nxdt_host.py         v0.6, from the rewrite branch
#
# STILL NEEDED ONCE, ON THIS MACHINE: the libusbK driver, via Zadig, bound to VID 0x057E
# PID 0x3000 (the console while nxdumptool is waiting for a host). PyUSB cannot see the device
# without it, and WinUSB is not a substitute -- the upstream README is explicit that libusbK is
# the one that works with PyUSB.
#
# usage: bash tools/nxdt-host/run.sh [outdir]
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="${1:-$HERE/out}"
mkdir -p "$OUT"

PY="$HERE/.venv/Scripts/python.exe"
[ -x "$PY" ] || { echo "no venv at $PY -- run: py -m venv $HERE/.venv" >&2; exit 1; }
[ -f "$HERE/libusb-1.0.dll" ] || echo "warning: libusb-1.0.dll missing next to the script" >&2

echo "receiving into $OUT"
echo "waiting for the console -- start the dump in nxdumptool now."
cd "$HERE"
exec "$PY" -u nxdt_host.py --cli --outdir "$OUT" --verbose
