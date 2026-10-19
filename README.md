# SSBU Mods: Rat of Doom

A moveset mod for Super Smash Bros. Ultimate. **Rat of Doom** is a Pikachu costume whose moves
are taken from over a dozen other fighters and reproduced as closely to the originals as the
engine allows: the same frame data, hitboxes, effects, sounds, and, where a move has its own
status logic, a transcription of the game's own code.

Its specials are Piranha Plant's Ptooie (neutral-B), Incineroar's Alolan Whip (side-B), Game &
Watch's Fire (up-B), and Incineroar's Revenge (down-B).

It runs as an [ARCropolis](https://github.com/Raytwo/ARCropolis) mod plus a Skyline plugin
built on [smashline](https://github.com/HDR-Development/smashline), on its own added costume slot,
so the other Pikachu costumes stay vanilla.

## What is here

| Path | What it is |
|---|---|
| `pikachu-moveset/plugin/` | The Skyline plugin (Rust): status code, ported scripts, articles, and telemetry |
| `pikachu-moveset/mod/` | The ARCropolis mod tree the build fills in |
| `pikachu-moveset/docs/` | Design notes and decision records for the moveset |
| `tools/` | The build: generators that port moves from the game's own data, the check suite, and deploy |
| `handbook/` | What the project has learned about porting one fighter's move onto another |
| `mod-builder/` | Notes and experiments toward a general moveset builder |
| `releases/` | Plugin builds from earlier milestones |

## Building

This is a working development tree rather than a packaged release. The build (`tools/build.sh`,
then `tools/check.sh`) generates most of the mod from files extracted from your own copy of the
game, and from third-party tools and dumps kept under `vendor/`. **Neither is included**: no game
assets are in this repository or its history.

## Notice

This is a fan project, not affiliated with or endorsed by Nintendo. Super Smash Bros. Ultimate
and its characters are trademarks of Nintendo and their respective owners. Some source files
transcribe or quote the game's own scripts and code for the sake of interoperability; those
portions remain the property of their owners, and the license below covers this project's own
work.

## License

[MIT](LICENSE).
