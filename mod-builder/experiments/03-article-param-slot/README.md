# Experiment 03 — confirm the article-clone crash mechanism without crashing

**Settles:** the one untested inference in `handbook/open-questions.md` §3b. The article clone
crash is root-caused statically; this closes the loop observationally.

**Status: prepared, NOT applied.** It adds a log line to
`pikachu-moveset/plugin/src/telemetry/`, which another session is actively building. Do not apply
while that is running. Checked 2026-09-14: `CLONE_ENABLED` is still `false` and nothing in the
plugin reads this address, so this is not duplicated work.

## What is already established

`clone_weapon` puts a real `WEAPON_KIND_PACKUN_SPIKEBALL` object on the host. Its native
`WeaponSpecializer_PackunSpikeball` init reads Piranha Plant's per-fighter-kind param object out of
a fixed slot and dereferences it:

```
0x3476e38  ldr  x20, [x8, #0x1228]    ; entry[FIGHTER_KIND_PACKUN].param_object
0x3476e44  ldp  x8, x9, [x20, #0x138] ; Data Abort, Address 0x138, X[20] = 0
```

`0x70 + 0x38 × 0x51 = 0x1228` and `FIGHTER_KIND_PACKUN = 0x51`, both verified. Every entry in that
94-slot array is created in exactly one place, keyed on the fighter kind currently loading, so with
no Piranha Plant in the match `entry[0x51]` is null forever.

**The single untested link:** that the crashing test match contained no Piranha Plant. The register
dump proves the slot was null and the code proves only a packun load fills it, but no roster log
was ever read.

## The probe

Read the slot and log it. No clone, no crash, no risk.

```rust
// telemetry, once on the first frame of the host object
let text = skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as u64;
// lib::Singleton<app::FighterParamAccessor2>::instance_ holds a POINTER at main+0x52bc3b0
let accessor = *((text + 0x52bc_3b0) as *const u64);
// entry[kind].param_object lives at +0x70 + 0x38*kind; kind 0x51 = PACKUN
let packun = if accessor != 0 { *((accessor + 0x1228) as *const u64) } else { 0 };
crate::telemetry::sink::diag(format!(
    "[rat] paramaccessor={accessor:#x} packun_slot={packun:#x}"));
```

Guard the inner read on `accessor != 0` — the singleton is built once during fight
initialisation, and reading through it too early is the one way this probe could itself fault.

## The two runs

| run | expectation |
|---|---|
| a match with **no** Piranha Plant | `packun_slot=0x0` |
| a match **containing** a Piranha Plant | `packun_slot=` some non-zero pointer |

Non-zero exactly when packun is present confirms the mechanism observationally, and the diagnosis
stops resting on an assumption about a months-old test match.

## What each outcome licenses

- **As expected** → the mechanism is confirmed from both directions. The fix under investigation
  elsewhere (force-creating the original owner's param entry, via the creator at `main 0x70c580`
  that smashline already hooks) is aimed at the right thing.
- **Null in both runs** → the slot is not what fills it, and the static reading has a gap. Re-read
  the creator's caller.
- **Non-zero in both runs** → something else already creates the entry, and the crash has a
  different cause. That would be the most informative failure, and it would invalidate part of
  §3b — which should then be corrected rather than quietly amended.

## Why it is worth doing even though the fix is progressing elsewhere

ADR 0005 of the moveset context forbids a fix without evidence. The mechanism is read, but one link
in the chain is an assumption, and this is the cheapest way to remove it — a log line and two
matches, against a clone experiment that costs a boot crash when it is wrong.
