# ADR 0005 — No fix without evidence, and no parallel lists

Status: accepted, 2026-09-11, after being told the work had been ping-ponging between the same
problems for some time. It had.

## The pattern, stated plainly

Revenge was implemented wrongly five times. The Alolan Whip's catch, four. The cargo carry, four.
Each round looked like progress and produced a playtest report saying it was still wrong. Reading
the sequence back, every one of those rounds had the same shape:

1. form a hypothesis from params, ACMD scripts, a wiki page, or reasoning about what the engine
   probably does;
2. implement it;
3. ship it;
4. learn one bit — "still wrong" — and start again.

One bit per round, against a hypothesis space that was not small. That is the ping-pong, and it is
not a matter of being more careful. It is a measurement problem.

## What broke it

Two things, and neither is "try harder".

**An instrument that yields the whole answer in one round.** The whip's per-frame probe prints
every input the catch decision reads, for us and for every opponent, on every frame of the window.
One playtest then contains this:

```
f=17.0 ... pullin_hit=true  them:[0x10000 st=0x2a cap=false fwd=5.0]
f=18.0 ... pullin_hit=true  them:[0x10000 st=0x2a cap=false fwd=12.8]
f=19.0 ...                  them:[0x10000 st=0x2a cap=false fwd=16.1]
```

which is not a hint, it is the answer: the opponent is in range on frame 17, our pull-in hitbox
launches them, and the window opened on 18. No hypothesis was required. The same round, the same
way: the cargo freeze was 7965 identical log lines with `st=0x1f7` in them, and Revenge's was two
lines naming the status the boost was spent in.

**Reading the game instead of inferring it.** `tools/disasm` plus `lua2cpp_common.nro` turned
"which common statuses are safe to put a victim into" from a thing you discover by crashing into
a thing you look up. See ADR 0004 and `docs/reading-the-games-code.md`.

## The rules

**1. A fix must cite a log line, an instruction, or a param read out of the game.** Not a name
that sounds right, not a wiki sentence, not "this is probably how it works". If none of those is
available, the round's job is to build the instrument that would produce one — and to say so,
rather than shipping a guess and calling it a fix.

**2. If a comment says something was measured, the measurement must exist.** A comment here
asserted `is_catch` never fires, "measured across a whole session". Every line of the log it
referred to stopped one frame before the window opened. An unfalsifiable claim in a comment is
worse than no comment: it forecloses the question.

**3. No parallel lists.** This project has now been bitten by the same shape five times: throw_f
missing from the drop-in pre-flight, DONKEY missing from deforeign's SOURCES, every special
missing from topscale's MAP, motions.rs drifting from ported-motions.txt, and — this round —
`in_counter_attack` still testing `status == SPECIAL_LW` after the retaliation moved to
SPECIAL_LW_HIT. Two facts that must agree, stored twice. Either derive one from the other, or put
them behind one named function, or add a check that compares them.

**4. A catastrophic failure mode gets a structural guard, not a fix.** Individual cycles are
endless; the class is not. So:

| mode | guard |
| --- | --- |
| same-frame status cycle → frozen game | `victim::allow_shift`, 8 per frame, reset from the one place that knows a frame passed |
| engine state left armed outside its status | `watch_absorb_leak`, shadowing our own writes because the engine exposes no getter |
| a handle to another battle object going stale | `victim::boma`, which makes the accessor prove its own id |
| a victim handed to code that needs an article we lack | `victim::FORBIDDEN_VICTIM_STATUSES` + `tools/check-victim.sh` |

Each of those converts a symptom that costs a playtest into a log line that costs nothing.

**5. When a guess is unavoidable, guess ONE thing.** Three simultaneous changes and one bit of
feedback is how a round gets wasted. Instrument the other two and change the one.

## What this does not license

It does not license stopping. The rule is not "wait for perfect information" — it is "spend each
playtest on a measurement rather than on a hypothesis". Rounds should still ship, and every round
should leave the next one better instrumented than the last: that is why the flush interval,
the breadcrumbs, the self-test's second pass, the status-graph check and the animation budget all
exist. Every one was added the round after the thing it would have caught.
