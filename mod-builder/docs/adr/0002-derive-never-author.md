# ADR 0002 — Derive, never author; one escape hatch, and it must cite a recording

Status: accepted, 2026-09-14.

## The decision

Any fact the builder needs that can be read out of the game **must** be read out of the game. No
per-move authored data, no override file.

The single exception is a `measured/` file, for facts that provably are not in the game's data.
Every entry cites the recording it came from, and a check rejects an entry once the value turns out
to be derivable.

This **replaces the *parity table*** — the moveset context's single hand-edited record of
source-of-truth hitbox values, from which its scripts are generated. Same role in the
architecture, opposite provenance. The builder's equivalent is the *requirements manifest*, and it
is computed.

## Why, in three pieces of evidence

**Parallel lists drift, and a checker is not a fix.** `pikachu-moveset` keeps its port list in
three places — `gen-moves.sh`'s inline groups, `tools/moves.tsv`, and `topscale`'s `MAP` — which
have to be held in step by hand. `tools/check-lists.sh` exists only to detect when they aren't.
Two commits claimed a move "had no ACMD at all", inferred from `moves.tsv`; `git show` proved both
were already installed, and the root cause was that `gen-moves.sh` keeps its own list.

**A hand-copied constant can be wrong in a way that no check can see.** Two fighter `scale`
constants were transcribed wrongly — Cloud 1.03 against an actual 0.98, and King K. Rool 1.15
against an actual 0.89, a 29% over-scale on a shipped move. The hitbox trace reported **0.00
error the entire time**, because it applied the same wrong constant to both sides of its own
comparison. The fix was not a better check; it was generating `scales.tsv` from the game's own
`fighter_param.prc`.

**And the one honest exception is real.** The Alolan Whip's rope holds its victim for **20
frames**. That number is in no param object — it belongs to the rope article, and the whole
`param_special_s` object was dumped to confirm its absence. It was measured from three native
recordings that agree to the frame. A rule of "never author anything" would have forced either a
guess or an abandoned move, and this project's standing rule is that guessing is worse than an
honest gap.

## Considered and rejected

**No escape hatch at all.** The right instinct, wrong at exactly one point: some facts are not in
the data, and there is a documented instance. A tool that cannot express one would push the
operator to encode it somewhere worse — a constant in the generator, which is authored data with
no citation and no check.

**Derive by default, with a general per-move override file carrying a reason string.** This is the
same thing as no rule at all, on a delay. Three parallel lists is what "just add an override"
looks like after six months, and this repository is the evidence.

## Consequences

- A wrong derivation must be fixed **in the deriver**, which is slower per incident and correct
  across every move at once.
- `measured/` needs a check with teeth: an entry whose value later appears in the corpus is an
  error, not a stale comment.
- The manifest is the only input to codegen, so anything the builder cannot derive is visible as a
  refusal rather than papered over by a default.
