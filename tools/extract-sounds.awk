# Generates a ported move's `sound_` script from the source's, with every label passed through
# tools/sound-map.tsv. The sound counterpart of tools/extract-effects.awk, built the same way and
# for the same reason: the dumps are not reliably compilable, so the body is GENERATED from the
# calls this recognises -- each PLAY_* or STOP_SE macro, kept at its frame -- rather than copied.
#
#   awk -v want=sound_attack11 -v map=tools/sound-map.tsv -v loops="attack100" -v fname=attack11 \
#       -f tools/extract-sounds.awk <source dump>
#
# WHAT HAPPENS TO A LABEL, in order:
#   * se_common_*, se_pikachu_*, vc_pikachu_*, seq_pikachu_*  kept -- every fighter has the common
#     bank, and the rest are Rat of Doom's own;
#   * a row in the map                                        rewritten, or dropped on "-";
#   * anything else                                           DROPPED and reported on stderr as
#     `UNMAPPED <label>`, so a new ported move with a sound nobody mapped fails loudly in the
#     generator's report instead of playing silence.
#
# LOOPS. A script named in `loops` is a rapid-jab-style loop the dump flattened. Measured on Little
# Mac's `sound_attack100` (lua2cpp_littlemac 0x710010cff0 -> 0x71001370d0, 2026-10-18): the binary
# is `do { frame(1); SE; wait(3); SE; wait(4); SE; wait(4); SE; wait(4); SE; wait_loop_sync_mot(); }
# while (true)`, and the dump prints the `wait_loop_sync_mot` FIRST. So the dump's leading sync is
# dropped and the body is wrapped in `loop { ...; wait_loop_sync_mot }` -- the same shape
# tools/restore-loop.awk gives the game script. smashline calls a script once; without the loop the
# rapid jab would make its sound for one cycle and then go quiet.
BEGIN {
    infn = 0; frame = 0; pending_frame = -1; kept = 0
    want_loop = (index(" " loops " ", " " fname " ") > 0)
    if (map != "") {
        while ((getline line < map) > 0) {
            if (line ~ /^#/ || line ~ /^[ 	]*$/) { continue }
            n = split(line, f, "	")
            if (n >= 2 && f[1] != "") { MAP[f[1]] = f[2] }
        }
        close(map)
    }
}

/^unsafe extern "C" fn / {
    infn = ($5 ~ "^" want "\\(")
    if (infn && want_loop) { print "    loop {" }
    next
}
!infn { next }

/wait_loop_sync_mot/ { next }

/frame\(agent\.lua_state_agent, / {
    if (match($0, /[0-9]+(\.[0-9]+)?/)) { frame = substr($0, RSTART, RLENGTH) + 0 }
    next
}
/wait\(agent\.lua_state_agent, / {
    if (match($0, /[0-9]+(\.[0-9]+)?/)) { frame = frame + substr($0, RSTART, RLENGTH) + 0 }
    next
}

{
    if ($0 !~ /macros::(PLAY_[A-Z_0-9]+|STOP_SE)\(agent, Hash40::new\("/) { next }
    line = $0
    if (!match(line, /Hash40::new\("[A-Za-z0-9_]+"\)/)) { next }
    label = tolower(substr(line, RSTART + 13, RLENGTH - 15))
    if (label ~ /^(se_common_|se_pikachu_|vc_pikachu_|seq_pikachu_)/) {
        out = label
    } else if (label in MAP) {
        out = MAP[label]
        if (out == "-") { dropped++; next }
    } else {
        print "UNMAPPED " label > "/dev/stderr"
        next
    }
    line = substr(line, 1, RSTART - 1) "Hash40::new(\"" out "\")" substr(line, RSTART + RLENGTH)
    if (frame != pending_frame) {
        if (pending_frame != -1) { print "    }" }
        if (frame > 0) { printf "    frame(agent.lua_state_agent, %s.0);\n", frame }
        print  "    if macros::is_excute(agent) {"
        pending_frame = frame
    }
    sub(/^[ \t]+/, "", line)
    printf "        %s\n", line
    kept++
}

END {
    if (pending_frame != -1) { print "    }" }
    if (want_loop) {
        print "    agent.clear_lua_stack();"
        print "    smash::app::sv_animcmd::wait_loop_sync_mot(agent.lua_state_agent);"
        print "    }"
    }
}
