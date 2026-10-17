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

# 1. The rope status, by constant name. Requested on anyone, this is a confirmed crash.
#
# NARROWED FROM THREE TO ONE, and the reason is that the crash was never about the rope.
#
# This checked THROWN, ATTACH_ROPE and RETURN. THROWN's danger was real -- its status opens with
# an unguarded `LinkModule::get_parent_pos` -- but the missing parent was `LinkModule::link` being
# called on one object instead of two, not the absent article. With both ends linked the ENGINE
# itself puts the victim into THROWN and nothing crashes. RETURN reads no parent at all: it is
# `reverse_lr`, a motion, four clears and the STOP energy from a float the victim's own CATCHED
# status stored from the captor's params.
#
# ATTACH_ROPE stays, because the rope IS that status. See the list in src/victim.rs for the
# addresses this was read from.
#
# src/victim.rs is allowed to name it -- it is the refusal list. Comments are allowed to discuss
# it, because the explanation is the point; only a request is a fault, so the grep looks for the
# constant inside a change_status call rather than for the name anywhere.
#
# NARROWED FROM ONE TO ZERO, AND THE CHECK CHANGED SHAPE BECAUSE OF IT.
#
# ATTACH_ROPE turned out to be safe too, and for the same reason the other two did: it was on this
# list from an assumption -- "the rope IS that status" -- rather than from a reading. Its three
# functions (common 0x710028dbb0, 0x710028df90, 0x710028e970) are `init_settings`,
# `set_swing_special_state`, `change_motion`, `set_rate`, `GroundModule::correct` and one
# `LinkModule::is_link`. Nothing dereferences anything. It is also a leg of the move the port was
# skipping, so refusing it was costing the snap into the ropes.
#
# WHICH LEAVES NOTHING TO GREP FOR, and a check that greps for an empty set is a check that always
# passes -- this one did exactly that for one commit, silently, because the request it was supposed
# to catch was written across three lines and the pattern was single-line. So it now guards the
# MECHANISM instead of the membership: `request_status` must still consult the list. Emptying the
# list is a decision with four disassemblies behind it; deleting the gate that reads it would throw
# away the ability to make that decision again.
# `grep -c` PRINTS 0 AND EXITS 1 WHEN IT FINDS NOTHING, so `|| echo 0` appends a second zero and
# `[ "0\n0" -lt 1 ]` is a syntax error that the `if` treats as false -- i.e. the check reported ok
# in both directions. Found by commenting the gate out and watching it pass anyway. `|| true`
# keeps grep's own count, which is already 0.
gate=$(grep -c "FORBIDDEN_VICTIM_STATUSES.contains" "$S/victim.rs" 2>/dev/null || true)
if [ "${gate:-0}" -lt 1 ]; then
    echo "  FAIL src/victim.rs no longer refuses anything: request_status does not consult"
    echo "       FORBIDDEN_VICTIM_STATUSES. The list is empty on purpose -- every status that was"
    echo "       on it was disassembled and cleared -- but the gate is how the next one gets"
    echo "       refused before it crashes the game."
    rc=1
else
    echo "  ok  request_status still consults FORBIDDEN_VICTIM_STATUSES (currently empty, by"
    echo "      evidence -- see the four disassemblies quoted above the list)"
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
    # THE FILE LIST IS COMPUTED FIRST, AND AN EMPTY ONE IS HANDLED, BECAUSE AN UNGUARDED `awk`
    # WITH NO FILE OPERANDS READS STDIN AND HANGS FOREVER.
    #
    # `victim_still_ours` was renamed out of the plugin at some point and nothing noticed: with no
    # match, `$(grep -rl ...)` expanded to nothing, `awk` fell back to stdin, and the whole
    # harness stopped at this gate. It only showed up when the harness was run with a terminal
    # attached -- under `nohup` stdin is /dev/null, so awk saw EOF and the gate passed in
    # silence. A check that hangs on one machine and passes vacuously on another is worse than
    # either.
    #
    # A missing function is now a NOTE rather than a failure. The gate's question is "does this
    # function ask the engine", and a function that does not exist cannot; but it is worth saying
    # out loud, because the list above is hand-maintained and this is how it goes stale.
    files=$(grep -rl "unsafe fn $fn" "$S" 2>/dev/null || true)
    if [ -z "$files" ]; then
        echo "  -   no \`unsafe fn $fn\` in the plugin; nothing to check for this one"
        continue
    fi
    body=$(awk -v f="unsafe fn $fn" '
        index($0, f) { inside = 1 }
        inside { print }
        inside && /^}/ { inside = 0 }
    ' $files </dev/null 2>/dev/null)
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
