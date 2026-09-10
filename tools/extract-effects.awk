# Extracts the engine-global `sys_*` effect spawns from a source fighter's effect script and
# emits a fresh, well-formed function body.
#
# WHY GENERATE RATHER THAN FILTER: the SSBU script dumps are not reliably compilable. R.O.B.'s
# effect_attacklw3 is a costume-colour branch table containing unresolved raw-address calls
# (`if(0x1050b0(*FIGHTER_INSTANCE_WORK_ID_INT_COLOR, 0)){`) and braces the decompiler mismatched
# outright -- an `else {` opened inside an unclosed `if`. Porting those verbatim and filtering
# what we cannot support fails to compile, and repairing decompiler output per script does not
# generalise. Emitting new code from only the statements we want cannot produce malformed output.
#
# WHAT IS KEPT: effect spawns whose archive is `sys_*`, which is always resident, so the mod stays
# CSK-free per ADR 0002. That is the smash-charge flash, hit impacts, smoke, speedlines and
# whirlwinds -- at the SOURCE's frame timings.
#
# WHAT IS DROPPED, and why:
#   - fighter-owned effects (`littlemac_attack_line`, `fox_smash_arc`, `koopa_scratch`): the
#     archive is not loaded for Pikachu.
#   - AFTER_IMAGE trails: they reference `tex_<fighter>_*` textures in the source's model.
#   - LAST_EFFECT_SET_* modifiers: they act on the last effect spawned, and every one of them in
#     these scripts sits inside a costume-colour conditional we are not reproducing. Keeping one
#     would recolour whichever sys_ effect happened to be spawned last.
#   - all conditionals, including region and costume branches.
#
# usage: awk -v want=effect_attacks4 -f extract-effects.awk <source.txt>
# Emits only the statements; the caller writes the fn signature and closing brace.

BEGIN { infn = 0; frame = 1; pending_frame = -1 }

/^unsafe extern "C" fn / {
    infn = ($5 ~ "^" want "\\(")
    next
}
!infn { next }

# Track the source's timeline so kept effects fire on the frame they originally did.
/frame\(agent\.lua_state_agent, / {
    if (match($0, /[0-9]+(\.[0-9]+)?/)) { frame = substr($0, RSTART, RLENGTH) + 0 }
    next
}
/wait\(agent\.lua_state_agent, / {
    if (match($0, /[0-9]+(\.[0-9]+)?/)) { frame = frame + substr($0, RSTART, RLENGTH) + 0 }
    next
}

{
    # Is this an effect-spawning macro? Structural: the macro name contains EFFECT and is not a
    # last-effect modifier. Enumerating the variants by hand missed EFFECT_FOLLOW_FLIP once.
    if ($0 !~ /macros::[A-Za-z_0-9]*EFFECT[A-Za-z_0-9]*\(/) { next }
    if ($0 ~ /LAST_EFFECT_SET_/) { next }
    if ($0 ~ /AFTER_IMAGE/) { next }

    # The first Hash40 argument is the effect archive.
    line = $0
    if (!match(line, /Hash40::new\("[a-z0-9_]+"\)/)) { next }
    first = substr(line, RSTART + 13, RLENGTH - 15)
    if (first !~ /^sys_/) { next }

    # One frame() per distinct frame, then the calls that belong to it.
    if (frame != pending_frame) {
        if (pending_frame != -1) { print "    }" }
        printf "    frame(agent.lua_state_agent, %s.0);\n", frame
        print  "    if macros::is_excute(agent) {"
        pending_frame = frame
    }
    sub(/^[ \t]+/, "", line)
    printf "        %s\n", line
    kept++
}

END { if (pending_frame != -1) { print "    }" } }
