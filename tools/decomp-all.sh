#!/usr/bin/env bash
# Decompile EVERY function in a module, once, into a per-function tree.
#
#   bash tools/decomp-all.sh common          8792 functions, the victim side of every grab
#   bash tools/decomp-all.sh donkey          re-dump a fighter at full coverage
#
# WHY, IN ONE SENTENCE FROM THE PLAYTEST THAT PAID FOR IT: "I don't want inference of any kind,
# reasonable or not, I want EXACT implementation."
#
# The cargo regrab bug cost five rounds and every one of them ended in an inference, because the
# transition that decides it -- `CATCH_WAIT -> SHOULDER_START` -- is in `lua2cpp_common.nro` and
# this repository had decompiled C for five FIGHTER NROs and none for common. Donkey Kong's own
# decomp has `ShoulderStart` onward and no `CatchWait`: he does not own that status, the common
# module does. So "how the source gets into the carry" was not merely unknown, it was UNREADABLE,
# and ADR 0006 -- transcribe, do not reimplement -- had nothing to transcribe. See its addendum.
#
# `tools/decomp.sh` could already reach any one function in common. Nobody could ask for the one
# they did not know to ask for. This removes the category.
#
# RESUMABLE, because 8792 functions is not one sitting and a stall must not cost the whole run.
# Every function already written is skipped, so re-running continues rather than restarts.
#
# HEARTBEAT ON EVERY BATCH, to `work/decomp/<module>.progress`, with a UTC stamp and the counts.
# A watcher reads that file: if the stamp stops advancing the run has stalled, which is a
# different failure from the run finishing, and the two used to be indistinguishable.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"

MOD="${1:?usage: decomp-all.sh <common|donkey|gaogaen|gamewatch|packun|pikachu|...> [batch]}"
BATCH="${2:-150}"

if [ "$MOD" = "main" ]; then
    # THE ENGINE ITSELF. `main`'s symbol dump is `addr size name` with no kind column -- it is not
    # an NRO and `decomp.sh` already special-cases it -- so the todo is built differently. Only
    # SYMBOLISED functions can be reached: without a full Ghidra auto-analysis, which takes hours
    # over 60 MB of AArch64, nothing knows where an unnamed function begins. 8905 named ones is
    # what there is, and it includes the capture and catch machinery this project keeps hitting.
    BIN="$ROOT/vendor/main/rat-main-text.bin"
    SYMS="$ROOT/vendor/main/symbols-merged.txt"
else
    BIN="$ROOT/vendor/nro/prebuilt/nro/release/lua2cpp_$MOD.nro"
    SYMS="$ROOT/vendor/main/$MOD-nro-symbols.txt"
fi
[ -f "$BIN" ] || { echo "no such binary: $BIN" >&2; exit 1; }

OUTDIR="$ROOT/vendor/decomp/$MOD"
WORK="$ROOT/work/decomp"
PROG="$WORK/$MOD.progress"
mkdir -p "$OUTDIR" "$WORK"

if [ ! -s "$SYMS" ]; then
    echo "building $SYMS"
    tools/disasm/target/release/disasm.exe "$BIN" --sym "" 2>/dev/null \
      | awk '$2=="def" || $2=="plt" { a=$1; sub(/^0x/,"",a); print a" 0 "$3" "$2 }' > "$SYMS"
fi

# EVERY DEFINED FUNCTION, and only defined ones: a `plt` entry is a call stub into another module
# and decompiling it yields a jump.
if [ "$MOD" = "main" ]; then
    awk '!/^#/ && NF>=3 && $1 ~ /^[0-9a-f]+$/ { print "0x"$1" "$3 }' "$SYMS" \
      | sort -u > "$WORK/$MOD.todo.all"
else
    awk '$4=="def" { print "0x"$1" "$3 }' "$SYMS" | sort -u > "$WORK/$MOD.todo.all"
fi
total=$(wc -l < "$WORK/$MOD.todo.all")

# NAMES ARE NOT UNIQUE, AND KEYING FILES ON THEM ALONE LOSES FUNCTIONS SILENTLY.
#
# `main` has 8905 symbolised addresses under 8055 distinct names -- 850 collisions, mostly C++
# overloads and per-template instantiations that share a mangled-away name. Writing
# `<name>.c` for each meant the second one overwrote the first and the run reported a tidy
# count that was 850 short with nothing to show which were missing. Duplicated names get the
# address appended; unique ones stay readable, which is almost all of them.
awk '{print $2}' "$WORK/$MOD.todo.all" | sort | uniq -d > "$WORK/$MOD.dupnames"
ndup=$(wc -l < "$WORK/$MOD.dupnames")
[ "$ndup" -gt 0 ] && echo "  $ndup duplicated symbol name(s) will carry their address"
# `addr sym filename`, computed once, so every site that touches a path agrees.
awk -v dups="$WORK/$MOD.dupnames" '
    BEGIN { while ((getline d < dups) > 0) { DUP[d] = 1 } }
    { print $1, $2, (DUP[$2] ? $2 "@" $1 : $2) }
' "$WORK/$MOD.todo.all" > "$WORK/$MOD.map"
mv "$WORK/$MOD.map" "$WORK/$MOD.todo.all"

# Skip what is already on disk, by the filename computed above, so this is an exact resume
# rather than a count.
: > "$WORK/$MOD.todo"
while read -r addr sym fname; do
    [ -s "$OUTDIR/$fname.c" ] || printf '%s %s %s\n' "$addr" "$sym" "$fname" >> "$WORK/$MOD.todo"
done < "$WORK/$MOD.todo.all"
todo=$(wc -l < "$WORK/$MOD.todo")
done_already=$((total - todo))

stamp() { date -u +%Y-%m-%dT%H:%M:%SZ; }
say() { printf '%s\n' "$*"; printf '%s\n' "$*" >> "$PROG"; }

say "[$(stamp)] $MOD: $total function(s), $done_already already decompiled, $todo to go, batch $BATCH"

batch_n=0
fail_n=0
while [ -s "$WORK/$MOD.todo" ]; do
    head -n "$BATCH" "$WORK/$MOD.todo" > "$WORK/$MOD.batch"
    n=$(wc -l < "$WORK/$MOD.batch")
    [ "$n" -eq 0 ] && break
    batch_n=$((batch_n+1))
    addrs=$(awk '{printf "%s ", $1}' "$WORK/$MOD.batch")

    # One Ghidra invocation per batch. `decomp.sh` writes a single annotated .c holding all of
    # them, delimited by `/* ==== <addr>  <symbol> ==== */`, which is what the split below reads.
    #
    # THE OUTPUT IS FOUND BY GLOB, NOT BY RECOMPUTING ITS NAME. `decomp.sh` names the file after
    # its first argument with `tr -d '0x'`, which deletes every `0` and `x` in the address rather
    # than the prefix -- `0x7100117050` becomes `711175`. Reproducing that here worked and then
    # silently did not, and fifteen batches in a row reported `wrote 0` while their output sat in
    # work/ghidra untouched. Depending on another script's filename arithmetic is the kind of
    # coupling that fails without saying so, so the directory is cleared first and whatever
    # appears is the output.
    rm -f work/ghidra/"$MOD"-*.annotated.c work/ghidra/"$MOD"-*.c
    ok=1
    if ! timeout 3600 bash tools/decomp.sh "$MOD" $addrs > "$WORK/$MOD.batch.log" 2>&1; then
        ok=0
        fail_n=$((fail_n+1))
        say "[$(stamp)] $MOD batch $batch_n: decomp.sh FAILED (see $MOD.batch.log)"
    fi

    # KEYED ON THE ADDRESS, NOT ON THE NAME IN THE HEADER.
    # `decomp.sh` strips the `_ZN7lua2cpp<n>` prefix when it annotates, so the name in the
    # delimiter is NOT the symbol in the table. Splitting on it wrote every function twice --
    # once under the stripped name from the header and once as a "produced nothing" stub under
    # the mangled name the resume check looks for, which made resume a no-op and the tree twice
    # the size it should be. The address is the same in both places, so it is the join key.
    wrote=0
    for ANN in work/ghidra/"$MOD"-*.annotated.c; do
        [ -s "$ANN" ] || continue
        w=$(awk -v outdir="$OUTDIR" -v batch="$WORK/$MOD.batch" '
            BEGIN { while ((getline l < batch) > 0) { split(l, a, " "); SYM[a[1]] = a[3] } }
            /^\/\* ==== 0x[0-9a-f]+  .* ==== \*\/$/ {
                if (name != "") close(outdir "/" name ".c")
                name = SYM[$3]
                if (name == "") { name = $4 }
                print "/* " $3 "  " $4 " */" > (outdir "/" name ".c")
                n++
                next
            }
            name != "" { print >> (outdir "/" name ".c") }
            END { print n+0 }
        ' "$ANN")
        wrote=$((wrote + w))
        rm -f "$ANN" "${ANN%.annotated.c}.c"
    done

    # A function the decompiler genuinely refuses is retired with a stub, so a resume does not
    # retry it forever and the run can finish.
    #
    # ONLY WHEN THE BATCH ITSELF SUCCEEDED. Stubbing after a FAILED batch retires functions that
    # were never attempted -- a timeout or a wedged Ghidra would silently mark 150 functions
    # "produced nothing" and the resume would skip them, which is the exact shape of gap this
    # whole script exists to close.
    if [ "$ok" -eq 1 ]; then
        while read -r addr sym fname; do
            [ -s "$OUTDIR/$fname.c" ] || printf '/* %s  %s -- the decompiler produced nothing */\n' \
                "$addr" "$sym" > "$OUTDIR/$fname.c"
        done < "$WORK/$MOD.batch"
    fi

    # THE QUEUE ADVANCES BY WHAT IS ON DISK, NOT BY HOW MANY WERE ASKED FOR.
    #
    # `tail -n +$((n+1))` dropped the batch unconditionally, so a failed batch's 150 functions
    # were skipped for the rest of the run and only a later re-run would notice. Re-filtering
    # against the output directory means a failure costs one retry, not a hole.
    : > "$WORK/$MOD.todo.next"
    while read -r addr sym fname; do
        [ -s "$OUTDIR/$fname.c" ] || printf '%s %s %s\n' "$addr" "$sym" "$fname" >> "$WORK/$MOD.todo.next"
    done < "$WORK/$MOD.todo"
    if [ "$(wc -l < "$WORK/$MOD.todo.next")" -eq "$(wc -l < "$WORK/$MOD.todo")" ]; then
        say "[$(stamp)] $MOD batch $batch_n: made NO progress -- stopping rather than spinning"
        mv "$WORK/$MOD.todo.next" "$WORK/$MOD.todo"
        break
    fi
    mv "$WORK/$MOD.todo.next" "$WORK/$MOD.todo"
    left=$(wc -l < "$WORK/$MOD.todo")
    say "[$(stamp)] $MOD batch $batch_n: asked $n, wrote $wrote, $left left, $fail_n batch failure(s)"
done

say "[$(stamp)] $MOD: DONE -- $(ls "$OUTDIR" | wc -l) file(s) in vendor/decomp/$MOD, $fail_n batch failure(s)"

# ---------------------------------------------------------------------------------------------
# WHAT THIS CANNOT REACH, WRITTEN DOWN SO THE GAP IS KNOWN RATHER THAN DISCOVERED
#
# A FIGHTER NRO EXPORTS ELEVEN SYMBOLS. `lua2cpp_donkey.nro` is 1.5 MB of code and its dynamic
# symbol table names exactly eleven functions, all of them `create_agent_fighter_*_donkey`
# entry points. Every status function in it is STATIC: reachable only by following the
# registration calls those eleven make. So `decomp-all.sh donkey` writes eleven files and is
# telling the truth -- there is nothing else it can name.
#
# `vendor/ssbu-status/ghidra_scripts/` holds the recovered per-status trees for five fighters,
# produced by walking those registrations (commit 3a885a4). That recovery is the missing step
# for the other fifteen, and it is a different job from this script: find the addresses first,
# then decompile them. This script will decompile any address list it is given, so the two
# compose once the recovery runs.
#
# `common` is the opposite case and that is why it went first: it is the shared library every
# fighter calls into, so all 8792 of its functions are exported and all 8792 are now on disk.
# The victim side of every grab lives there, which is what five rounds of the cargo bug needed.
#
# `main` exposes 8905 symbols out of 60 MB of AArch64. Everything past those needs a full Ghidra
# auto-analysis to find function boundaries at all -- hours, and a different mode of this tool.
