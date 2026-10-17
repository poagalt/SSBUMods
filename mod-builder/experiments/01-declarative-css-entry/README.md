# Experiment 01 — can a character be added to the CSS declaratively?

**Settles:** [ADR 0007](../../docs/adr/0007-the-css-entry-is-data-not-code.md), which is *accepted
pending this result*. If the entry appears, the builder emits a JSON file for every character's
CSS identity and never writes a plugin for it. If it does not, the builder emits two CSK API calls
instead, the way all four surveyed mods do.

**Cost:** one folder, one boot. Fully reversible by deleting the folder.

## Prerequisite

**The CSK Collection must be installed on the console.** It is what scans `mods:/database/`; with
it absent this experiment tests nothing and produces no error. Version 5.0.6 is extracted at
`C:\Users\poaga\Downloads\the_csk_collection_5_0_6\` — its plugin goes to
`sd:/atmosphere/contents/01006A800016E000/romfs/skyline/plugins/libthe_csk_collection.nro`.

## Install

Copy this directory's `database/` folder into any enabled mod folder, e.g.

```
sd:/ultimate/mods/ratbuilder-db-test/database/ratbuilder_test.json
```

`mods:/database/` is ARCropolis's **merged** mod filesystem, so the containing folder's name does
not matter — the same way CSK's `flags/` features work. The filename does not matter either: CSK
scans the directory and feeds every file to `serde_json` with no extension filter.

Optionally also create the zero-byte `sd:/ultimate/parse_json_database.flag`. The scan is **not**
gated on it — that check reads an in-memory bool which `main` sets unconditionally — but what the
flag does was not determined, and it may enable logging that makes a negative result legible.

## What to look for

Boot to the character-select screen and count the roster.

| outcome | meaning |
|---|---|
| A **new entry appears** (even with a broken or missing portrait) | the declarative path works. ADR 0007 becomes accepted outright. |
| The roster is **unchanged** | the path is incomplete or the schema guess is wrong. Check the Skyline log for `[CSK Collection::parse_json_database] Failed parsing file … ! - Reason: …`, which names the offending field. |
| The CSS **crashes** | the row was appended and something downstream needs assets we did not ship. That still answers the question — the append worked. Delete the folder to recover. |

**Do not select the entry.** It clones Pikachu's row and declares `color_num: 1`, but no slot
assets exist behind it, so selecting it is undefined. The experiment only needs the roster count.

## Why the file looks like this

Modelled on a **working** sibling rather than on the schema alone:
`thwomp_moveset_release__/Thwomp Palace Release/database/dossun_palace.json` registers BGM and
stage rows through the same `Config` struct. Conventions taken from it:

- ids are bare hash40 **strings** carrying their prefix (`ui_chara_…`, `ui_layout_…`), not hex
- `clone_from_<x>_id` is how a new row inherits an existing one's columns
- **entries are sparse** — that file's bgm entry sets 7 fields out of a much larger struct, so a
  character entry does not need all ~40

The field names come from `CharacterDatabaseEntrySerde` and `CharacterLayoutDatabaseEntrySerde`,
read out of `libthe_csk_collection.nro` 5.0.6.

**The guessed part, stated plainly:** that `fighter_kind` takes the string `fighter_kind_pikachu`
rather than an integer, and that `color_num` is an integer. Both follow the prc field conventions
and the sibling file's style, and neither is confirmed. A parse failure names the field, so a
wrong guess here is one iteration, not a dead end.

## If it works, the next question

`mods:/flags/reslot.toml` — `CharacterInfo { color_start_index, color_num, original_ui_chara_id }`
— is the other half: it maps the new character onto a host's slot range. This experiment
deliberately omits it, so that a positive result isolates *the append* from *the slot mapping*.
