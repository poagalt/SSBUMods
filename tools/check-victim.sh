#!/usr/bin/env bash
# No hand-written code may put another fighter into a status that dereferences an article we do
# not ship, and no module may reach another battle object without validating the handle.
#
# WHY THIS CHECK EXISTS. Every crash this moveset has produced is one mistake in two costumes:
# we handed a victim, or ourselves, to engine code written for a different fighter. The clearest
# case is proven by an Atmosphere report and by the game's own binary:
#
#   Data Abort, fault address 0
#     L2CFighterCommon::status_SwingGaogaenThrown + 0xf8
#     -> app::lua_bind::LinkModule__get_parent_pos_impl
#
# `tools/disasm` on `lua2cpp_common.nro` shows `get_parent_pos` returning the LINKED PARENT's
# position -- for Incineroar always his ROPE article -- and the next three instructions reading
# x, y and z off it with no null check. Pikachu has no rope, so the pointer is null.
#
# The contrast is what makes this checkable rather than a judgement call: `SwingGaogaenCatched`
# calls `LinkModule::is_link` FIRST and is therefore safe. Safety here is a property of the
# individual status, so the list of unsafe ones lives in src/victim.rs and this asserts nobody
# goes around it.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
S=pikachu-moveset/plugin/src
rc=0

# 1. The rope statuses, by constant name. Requested on anyone, this is a confirmed crash.
#
# src/victim.rs is allowed to name them -- it is the refusal list. Comments are allowed to
# discuss them, because the explanation is the point; only a request is a fault, so the grep
# looks for the constant inside a change_status call rather than for the name anywhere.
ROPE='FIGHTER_STATUS_KIND_SWING_GAOGAEN_(THROWN|ATTACH_ROPE|RETURN)'
hits=$(grep -rnE "change_status[a-z_]*\(.*$ROPE" "$S" 2>/dev/null | grep -v '^.*/victim\.rs:' || true)
if [ -n "$hits" ]; then
    echo "  FAIL a rope status is being requested -- this is the confirmed null deref:"
    printf '%s\n' "$hits" | sed 's/^/       /'
    rc=1
else
    echo "  ok  no code requests SWING_GAOGAEN_THROWN / _ATTACH_ROPE / _RETURN"
fi

# 2. Reaching another battle object goes through src/victim.rs.
#
# `sv_battle_object::module_accessor` on an id that is stale, recycled or simply not a fighter
# returns a pointer the engine will make a virtual call through, and the resulting Instruction
# Abort names no function -- the least diagnosable crash this project has had. victim::boma makes
# the accessor prove it knows its own battle_object_id before anyone gets to use it.
#
# Comment lines are excluded. The explanation of why this is dangerous belongs next to the code
# that used to do it, and a check which forbids writing that explanation down is a check somebody
# eventually deletes.
raw=$(grep -rn "sv_battle_object::module_accessor" "$S" 2>/dev/null \
      | grep -v '^.*/victim\.rs:' \
      | grep -vE '^[^:]+:[0-9]+: *//' || true)
if [ -n "$raw" ]; then
    echo "  FAIL another object is being reached without validation; use crate::victim::boma:"
    printf '%s\n' "$raw" | sed 's/^/       /'
    rc=1
else
    echo "  ok  every cross-object handle goes through crate::victim::boma"
fi

# 3. "Is the victim still ours" must not be asked of the engine.
#
# THIS IS THE REGRESSION GUARD FOR THE BUG THAT BROKE BOTH GRABS AT ONCE. The whip and the carry
# each had a `still_held` test written against `CaptureModule::is_capture` and `catch_object_id`,
# or against the victim's own status. Both are false within a frame of a hand-made catch -- the
# log is unambiguous: `victim 0x10000 stays in status 0x44` with `cap=false by=0x50000000` on
# every frame of every attempt -- because the engine refused the catch and keeps no record of it.
#
# The whip cancelled itself into SPECIAL_S_END ("a failure animation plays") and the carry dropped
# the victim as soon as the engine's own throw ended ("the opponent falls off after a second").
# Ownership is now a fact in src/victim.rs with its own ceiling.
#
# SCOPED TO THE OWNERSHIP FUNCTIONS THEMSELVES, not to the whole file. The first version of this
# grepped for the CaptureModule calls anywhere outside victim.rs and reported four false
# positives, every one legitimate: `find_caught_victim` asks the engine whether IT caught
# someone, which is tier one of the catch; `find_victim_in_range` skips anyone already captured
# by anybody; and the per-frame probe logs both. Asking the engine is fine for DETECTING a catch
# and for logging. It is only wrong as the oracle for "are we still holding them".
#
# So the check reads the bodies of the functions that answer that question, and nothing else. A
# check that cries wolf four times out of four gets deleted -- see ADR 0005, which says the same
# thing about the status-graph reachability test.
own=""
for fn in victim_still_ours still_held; do
    body=$(awk -v f="unsafe fn $fn" '
        index($0, f) { inside = 1 }
        inside { print }
        inside && /^}/ { inside = 0 }
    ' $(grep -rl "unsafe fn $fn" "$S") 2>/dev/null)
    # Matched in bash rather than through a pipe -- see the note in tools/check.sh step 3j.
    if [[ "$body" =~ CaptureModule::(is_capture|catch_object_id)|StatusModule::status_kind ]]; then
        own="$own $fn"
    fi
done
if [ -n "$own" ]; then
    echo "  FAIL these ask the ENGINE whether we still hold a victim; use crate::victim::held:$own"
    echo "       The engine refuses these catches and keeps no record of them -- measured, every"
    echo "       frame of every attempt: 'victim stays in status 0x44' with cap=false. Asking it"
    echo "       cancelled the whip's throw and dropped the cargo victim."
    rc=1
else
    echo "  ok  ownership of a held victim comes from crate::victim, not from the engine"
fi

# 4. The telemetry must be able to survive a crash.
#
# This is a check because the value regressed silently once and cost a whole session: the sink
# buffered for 120 frames, so the run that crashed on the rope throw left a 365-byte log holding
# the install lines and nothing else. Every frame of the move that caused it was in the buffer.
f=$(grep -oE 'const FLUSH_FRAMES: u32 = [0-9]+' "$S/telemetry/sink.rs" | grep -oE '[0-9]+$')
if [ -z "$f" ]; then
    echo "  FAIL cannot find FLUSH_FRAMES in telemetry/sink.rs"
    rc=1
elif [ "$f" -gt 15 ]; then
    echo "  FAIL FLUSH_FRAMES is $f -- a crash would destroy $f frames of evidence (want <= 15)"
    rc=1
else
    echo "  ok  telemetry flushes every $f frame(s), so a crash loses less than a move"
fi

exit $rc
