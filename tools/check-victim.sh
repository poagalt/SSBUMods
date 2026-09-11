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

# 3. The telemetry must be able to survive a crash.
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
