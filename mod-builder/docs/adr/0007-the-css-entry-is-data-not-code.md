# ADR 0007 — The character's CSS entry is emitted as data, not as plugin code

Status: **accepted, pending one experiment**, 2026-09-14.

## The decision

To put a generated character on the character-select screen, the builder emits
**`mods:/database/<char>.json`** and **`mods:/flags/reslot.toml`**. It does not emit a plugin that
calls The CSK Collection's registration API for this purpose.

## Why this needs an ADR: every shipped mod does the opposite

All four slotted characters examined — Waluigi, Thwomp, Herobrine, Gogeta — register their CSS
entry by **calling CSK's runtime API from their own `plugin.nro`**. None of them uses the
declarative path. A future reader who compares the builder's output against the entire published
corpus will conclude the builder is doing it wrong, and "fix" it by emitting plugin calls.

So the reasoning has to be on the record.

## What is actually required, which is much less than it looks

The four mods import between 7 and 13 CSK symbols, which makes the dependency look heavy. It is
not. Measured across all four:

| | |
|---|---|
| `add_chara_db_entry_info` | **required.** Appends the `ui_chara_db` row; a `.prcxml` can patch an existing row by index but **cannot append one**. |
| `add_chara_layout_db_entry_info` | **very likely required.** A separate registration queue, so the first call does not create a layout row implicitly, and 4 of 4 call it. |
| everything else | **optional, proven by counterexample within the sample** — Thwomp ships with no BGM block and no online permission. The rest is music, series art, announcer, victory jingle, tips, and Gogeta's transformation gimmick. |

So the real dependency is **one confirmed call, two in practice.** And CSK's `Config` schema, read
out of the plugin, has `character_database_entries` and `layout_database_entries` as **its first
two keys** — precisely the two things needed.

## Why data rather than the proven code path

- **The schema is richer than the API.** `CharacterDatabaseEntrySerde` carries `is_patch`,
  `marker_file` and `marker_fighter_folder` — none of which has a counterpart in the FFI calls.
  That reads like the newer, intended interface rather than a vestigial one.
- **It composes with [ADR 0002](0002-derive-never-author.md).** A JSON file the builder generates
  from the manifest is inspectable, diffable, and editable by the user. A compiled plugin call is
  none of those.
- **It removes a whole emission target.** Under this decision the builder emits Rust only for
  *moves*; the character's identity is data. That is one less thing for the transpiler in
  [ADR 0004](0004-transpile-do-not-transcribe.md) to be responsible for.
- The CSK dependency does not go away — the plugin still has to be installed, because it is what
  reads `mods:/database/` and rebuilds the prc. Only the requirement that *our* output be code
  goes away.

## The experiment this is pending on

**No shipped mod exercises the declarative character path**, and `allow_ui_chara_hash_online` has
no `Config` counterpart, so the path is read in the code and unproven end to end.

The test is narrow and costs one file and one boot: write a `mods:/database/test.json` with a
single `character_database_entries` row plus its `layout_database_entries` pair, boot to the CSS,
and look. If the entry appears, this decision stands. If it does not, fall back to emitting the
two API calls — and record that here rather than silently.

Until that result exists, this ADR is **accepted pending** rather than accepted.

## Consequences

- The builder needs no Rust at all for a character that ports only class-1 and class-2 moves whose
  ACMD it generates — the CSS entry, the slot nodes, the params (ParamConfig TOML) and the config
  are all data. That is a materially simpler artefact than any shipped mod.
- The `is_patch` field means the same emitter can produce a *replacement* character (ADR 0006's
  opt-in) by flipping one boolean, rather than needing a second code path.
- If the experiment fails, the fallback is known, small, and proven by four mods — so the downside
  is an evening, not a redesign.
