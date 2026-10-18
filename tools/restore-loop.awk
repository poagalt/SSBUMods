# Restores the loop that the script dump lost, for scripts named in LOOP_SCRIPTS (gen-moves.sh).
#
# THE EVIDENCE, because this changes control flow and nothing else in the generator does.
# SSBU-Dumped-Scripts renders compiled lua2cpp as straight-line Rust, and a `do { ... } while (true)`
# does not survive it. Little Mac's `game_attack100` is registered at 0x71000a3800 in
# lua2cpp_littlemac, and that address is an eight-instruction stub:
#
#     str x19,[sp,#-0x20]! / stp x29,x30 / add x29 / mov x19,x0 / mov x0,x1
#     BL 0x7100146cc0        Variadic::get_format -- argument setup
#     mov x0,x19
#     BL 0x71000f8130        the loop, never returns
#
# No frame-0 hit, no initial park. 0x71000f8130 decompiles to
#
#     do {
#         frame(1);  hit()      frame(3);  hit()   ...   frame(19); hit()
#         wait_loop_clear();
#     } while (true);
#
# where each hit() is `if is_excute { ATTACK }; wait(1); if is_excute { clear_all;
# on_flag(100_CONTINUE_CHECK) }`. The dump has the same ten hits ROTATED by one block -- it begins at
# the frame-19 hit, printed without its `frame(19)` so it reads as a frame-0 hit, then the loop's
# trailing `wait_loop_clear`, then frames 1-17, then a dangling `frame(19)` -- and no loop. Every
# argument was right, which is why every parity gate passed: the VALUES matched and the CONTROL FLOW
# did not.
#
# THE TRANSFORM, which adds and drops no statement: everything before `wait_loop_clear` moves to the
# end, `wait_loop_clear` follows it, and the body is wrapped in `loop`.
#
#     in:   PRE  wait_loop_clear  POST
#     out:  loop { POST  PRE  wait_loop_clear }
#
# An infinite loop in an ACMD is what the game does here; the engine abandons the coroutine when the
# motion changes, which is how releasing the button hands over to attack_100_end.
#
# usage: awk -v loops="attack100 ..." -v fname=attack100 -f restore-loop.awk
BEGIN { want = (index(" " loops " ", " " fname " ") > 0) }

!want { print; next }

/^unsafe extern "C" fn / { print; inbody = 1; npre = 0; npost = 0; seen = 0; next }

inbody && /^}/ {
    if (!seen) {
        # No wait_loop_clear means this is not the shape the transform is for. Emit it untouched
        # rather than wrapping a non-looping script in a loop it was never meant to have.
        for (k = 1; k <= npre; k++) print pre[k]
        print
        inbody = 0
        next
    }
    print "    loop {"
    for (k = 1; k <= npost; k++) print post[k]
    for (k = 1; k <= npre; k++) print pre[k]
    print "    macros::wait_loop_clear(agent);"
    print "    }"
    print
    inbody = 0
    next
}

inbody && /wait_loop_clear\(agent\);/ { seen = 1; next }

inbody { if (seen) post[++npost] = $0; else pre[++npre] = $0; next }

{ print }
