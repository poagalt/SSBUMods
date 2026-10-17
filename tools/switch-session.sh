#!/usr/bin/env bash
# The next session with the Switch, prepared so it is one sitting. handbook/switch-session.md is
# the procedure; this is its tooling.
#
#   bash tools/switch-session.sh check            # what is ready on this PC -- run BEFORE the console is on
#   bash tools/switch-session.sh lists            # write the no-keys fallback pull lists (from the move index)
#   bash tools/switch-session.sh dump [outdir]    # receive nxdumptool's USB dump (tools/nxdt-host)
#   bash tools/switch-session.sh post-dump <dir>  # verify the dump, extract ALL of data.arc locally,
#                                                 #   rebuild the ELFs, the code index and the move index
#   bash tools/switch-session.sh pull <list>      # the fallback with no keys: one list from the RUNNING game
#
# WHAT A SWITCH IS STILL NEEDED FOR, in one line each (the doc has the detail):
#   prod.keys    only Lockpick_RCM on the console can produce them; they gate everything below
#   data.arc     16.39 GiB; this PC has 3.6% of it -- every model, texture, effect archive and sound
#   ExeFS        the real main NSO and the SDK modules main imports from, instead of memory dumps
#   two NROs     lua2cpp_item and lua2cpp_trail (Sora), not among the 96 on this PC
#   hardware     playtests and telemetry; nothing offline substitutes for the game running
set -uo pipefail
export MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*'
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
DATA_ARC_BYTES=17600880952          # rom:/data.arc as the running game reports it (work/arc-raw.err)
ARC_BULK=tools/arc-pull/target/release/arc-bulk.exe
SESSION=work/switch-session

ok()   { printf '  ok    %s\n' "$*"; }
miss() { printf '  TODO  %s\n' "$*"; }

cmd_check() {
    echo "== this PC, for the dump route (prod.keys + nxdumptool)"
    [ -x tools/nxdt-host/.venv/Scripts/python.exe ] && ok "nxdt-host venv" || miss "nxdt-host venv: py -m venv tools/nxdt-host/.venv; pip install pyusb tqdm libusb-package"
    [ -f tools/nxdt-host/libusb-1.0.dll ] && ok "libusb-1.0.dll beside nxdt_host.py" || miss "libusb-1.0.dll"
    [ -f vendor/nxdt_rw_poc.nro ] && ok "nxdumptool .nro vendored (vendor/nxdt_rw_poc.nro)" || miss "nxdumptool .nro"
    local drv
    drv=$(powershell -NoProfile -Command "Get-PnpDevice -PresentOnly -ErrorAction SilentlyContinue | Where-Object InstanceId -match 'VID_057E&PID_3000' | ForEach-Object { (Get-PnpDeviceProperty -InstanceId \$_.InstanceId -KeyName DEVPKEY_Device_Service).Data }" 2>/dev/null | tr -d '\r')
    if [ -z "$drv" ]; then
        miss "libusbK driver: cannot tell until the console sits in nxdumptool's USB mode; then rerun check"
    elif [ "${drv,,}" = "libusbk" ]; then
        ok "console present, driver libusbK"
    else
        miss "console present but its driver is '$drv'; bind libusbK with Zadig (VID 057E PID 3000)"
    fi
    local free_gb
    free_gb=$(powershell -NoProfile -Command "[math]::Floor((Get-PSDrive C).Free/1GB)" 2>/dev/null | tr -d '\r')
    [ "${free_gb:-0}" -ge 60 ] && ok "C: has ${free_gb} GB free (a dump plus a full extraction needs ~60)" \
                               || miss "C: has ${free_gb:-?} GB free; a dump plus a full extraction needs ~60"
    local found
    found=$(find /c/Users/poaga -maxdepth 5 -name data.arc -size +17000000000c 2>/dev/null | head -1)
    [ -n "$found" ] && ok "a full-size data.arc already exists: $found" || echo "  --    no full data.arc on this PC yet (expected)"

    echo "== this PC, for what happens after the dump"
    for t in "$ARC_BULK" tools/motion-patch/target/release/mljson.exe tools/prc-tool/target/release/prcdump.exe \
             tools/disasm/target/release/disasm.exe; do
        [ -x "$t" ] && ok "$t" || miss "$t (cargo build --release in its directory)"
    done
    wsl.exe -d Ubuntu --exec test -x /opt/ghidra_12.1.4_PUBLIC/support/analyzeHeadless 2>/dev/null \
        && ok "Ghidra 12.1.4 in WSL (tools/rea-wsl.sh)" || miss "Ghidra in WSL: bash tools/rea-wsl.sh setup"
    [ -f work/move-index/ssbu-moves.sqlite ] && ok "move index built" || miss "python tools/move-index.py build"
    [ -f work/code-index/ssbu-code.sqlite ] && ok "code index built" || miss "bash tools/code-index.sh all"

    echo "== needs poagalt, not this script"
    echo "  -     Lockpick_RCM payload that supports HOS 23.0.0, from a source you trust (not vendored here)"
    echo "  -     the console, a USB-C data cable, and an RCM boot (handbook/switch-session.md, step 1)"
}

cmd_lists() {
    [ -f work/move-index/ssbu-moves.sqlite ] || { echo "build the move index first: python tools/move-index.py build" >&2; return 1; }
    mkdir -p "$SESSION"
    python - "$SESSION" <<'EOF'
import sqlite3, sys, os
out = sys.argv[1]
con = sqlite3.connect("work/move-index/ssbu-moves.sqlite")
q = lambda sql: [r[0] for r in con.execute(sql)]
missing = "local_path is null"
# In the order a short session should take them: smallest and most blocking first.
# NROs straight from the arc map: lua2cpp_item belongs to no fighter, so the asset table never sees it.
local = set()
for d in ("vendor/nro/prebuilt/nro/release", "work/arc-nro/prebuilt/nro/release", "extracted/prebuilt/nro/release"):
    if os.path.isdir(d):
        local |= set(os.listdir(d))
nros = [l.strip() for l in open("work/acquire/arc-paths.txt", encoding="utf-8", errors="replace")
        if l.strip().endswith(".nro") and os.path.basename(l.strip()) not in local]
lists = [
    ("01-nros", nros),
    ("02-effect-archives", q(f"select arc_path from assets where category='effect' and {missing} and arc_path like '%.eff'")),
    ("03-sora-c00-motion", q(f"select arc_path from assets where fighter='trail' and category='motion' and {missing} and arc_path like '%/c00/%'")),
    ("04-c00-models", q(f"select arc_path from assets where category='model' and {missing} and arc_path like '%/c00/%'")),
    ("05-effect-other", q(f"select arc_path from assets where category='effect' and {missing} and arc_path not like '%.eff'")),
    ("06-sound", q(f"select arc_path from assets where category='sound' and {missing}")),
    ("07-other-costumes", q(f"select arc_path from assets where category in ('model','motion') and {missing} and arc_path not like '%/c00/%'")),
]
for name, paths in lists:
    with open(os.path.join(out, f"pull-{name}.txt"), "w", newline="\n") as f:
        f.write("\n".join(sorted(set(paths))) + ("\n" if paths else ""))
    print(f"{out}/pull-{name}.txt  {len(set(paths)):>7} paths")
EOF
}

cmd_dump() {
    local out="${1:-$SESSION/dump}"
    mkdir -p "$out"
    exec bash tools/nxdt-host/run.sh "$out"
}

cmd_post_dump() {
    local dir="${1:?post-dump <dir with the nxdumptool output>}" arc main
    arc=$(find "$dir" -name data.arc 2>/dev/null | head -1)
    [ -n "$arc" ] || { echo "no data.arc under $dir" >&2; return 1; }
    local size; size=$(stat -c %s "$arc")
    if [ "$size" != "$DATA_ARC_BYTES" ]; then
        echo "data.arc is $size bytes, the game's is $DATA_ARC_BYTES -- an incomplete dump; stop here" >&2
        return 1
    fi
    echo "== data.arc: $arc, $size bytes (matches the running game's)"
    # The ExeFS: a file named `main` that starts with NSO0, or an exefs.nsp/PFS0 holding one.
    main=$(for f in $(find "$dir" \( -name main -o -name 'exefs*.nsp' \) 2>/dev/null); do
               head -c4 "$f" | grep -q 'NSO0\|PFS0' && echo "$f"; done | head -1)
    mkdir -p "$SESSION"
    echo "== extracting ALL of data.arc into extracted/ (existing files are kept; --dry-run first)"
    "$ARC_BULK" "$arc" extracted --dry-run 2>&1 | tail -3
    "$ARC_BULK" "$arc" extracted --skip-existing > "$SESSION/arc-bulk-local.log" 2>&1
    echo "   arc-bulk exit $? (log $SESSION/arc-bulk-local.log)"
    if [ -n "$main" ]; then
        echo "== the real main NSO: $main"
        python tools/rea-elf.py nso "$main" --name main-nso
    else
        echo "== no ExeFS main in the dump; work/rea/main.elf stays the memory-dump build"
    fi
    echo "== ELFs for every NRO, now including any the arc held that this PC lacked"
    python tools/rea-elf.py all > "$SESSION/rea-elf-all.log" 2>&1; echo "   rea-elf exit $?"
    echo "== move index"
    python tools/move-index.py build > "$SESSION/move-index.log" 2>&1; echo "   move-index exit $?"
    echo "== code index: analyse once (main ~50 min), export, load -- run in the background"
    echo "   bash tools/code-index.sh all"
}

cmd_pull() {
    local list="${1:?pull <list> (from: bash tools/switch-session.sh lists)}"
    echo "the game must be RUNNING with arc_network loaded; pulling $(grep -c . "$list") paths into extracted/"
    exec bash tools/arc-fetch.sh "$list" extracted
}

case "${1:-}" in
    check)     cmd_check ;;
    lists)     cmd_lists ;;
    dump)      shift; cmd_dump "$@" ;;
    post-dump) shift; cmd_post_dump "$@" ;;
    pull)      shift; cmd_pull "$@" ;;
    *)         sed -n '2,10p' "$0"; exit 2 ;;
esac
