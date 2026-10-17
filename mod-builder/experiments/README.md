# Experiments

Each one settles a decision that is currently accepted on a reading rather than on an observation.
They are small on purpose: one file or one log line, one boot, and a binary outcome.

| # | Settles | Touches | Status |
|---|---|---|---|
| [01](01-declarative-css-entry/) | [ADR 0007](../docs/adr/0007-the-css-entry-is-data-not-code.md) — is the CSS entry data or code? | nothing existing; a new standalone mod folder | **ready to run** |
| [02](02-share-instead-of-ship/) | decision 25 — can a ported animation be served from the game's own archive? | `pikachu-moveset/mod/` + `gen-config.sh` | prepared, **not applied** |
| [03](03-article-param-slot/) | `handbook/open-questions.md` §3b — the last untested link in the article-clone diagnosis | `pikachu-moveset/plugin/src/telemetry/` | prepared, **not applied** |

**02 and 03 are held deliberately.** Another session is actively building, deploying and
playtesting out of this same worktree — HEAD moved three commits and a deploy landed while these
were being written. Applying either would contend for the deploy lock and change what is on the
card underneath that session's playtest. 01 touches nothing either session owns, so it can go at
any time.

## One experiment was dropped rather than run

The plan previously called for raising a fighter's `color_num` from 8 to 9 and counting the colour
strip on the character-select screen, to find out whether slots past `c07` are possible.

**Superseded.** Four shipped mods create slots at `c120`, `c40`, `c32` and `c103`, which answers
the question more thoroughly than the experiment would have: the bound is not near 8, the slot
index is a byte, and `c%02d` is a minimum width rather than a fixed one. `color_num` for an added
character is declared in its own CSK entry, which experiment 01 exercises.

Its one useful by-product survived the cancellation: **ARCropolis 4.0.9 does honour `.prcxml`** —
`xml-rs` ×7 and `prcxml` ×3 in the binary — so a null result from a prcxml-based test would have
been interpretable. That caveat can be dropped from the plan.
