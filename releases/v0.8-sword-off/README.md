# v0.8-sword-off — the last state known to load and play

Snapshotted 2026-09-19 on request, before the dual-wield rework and the ten new moves.

**This is the build that ended four consecutive failed boots**, and it is the restore point to come
back to if the next round goes wrong. A three-stock match was played on it.

    plugin      librat_of_doom.nro   md5 8ad9dae9
    buildstamp  57faec4+dirty 2026-09-19T04:19:34Z
    mod tree    751 files, ARTICLE_SLOTS=c00

## What is in here

    librat_of_doom.nro    the exact binary that was on the card
    mod.tar.gz            the whole pikachu-moveset/mod tree, including config.json
    plugin-src.tar.gz     pikachu-moveset/plugin/src + Cargo.toml
    moves.tsv             the move table as it stood
    ported-motions.txt    the motion list as it stood

The `.nro` alone is what previous releases kept, and it is not restorable on its own: the mod tree
carries the article assets, the grafted meshes and the `config.json` declarations, and a plugin
without them is a different mod. Both halves are here.

## To restore

    tar -xzf releases/v0.8-sword-off/mod.tar.gz -C pikachu-moveset
    tar -xzf releases/v0.8-sword-off/plugin-src.tar.gz -C pikachu-moveset/plugin
    bash tools/build.sh && bash tools/deploy.sh

## The state it captures, precisely

Articles are ON with **`sword` (Chrom's Falchion) DISABLED** — that row is what crashed the game.
Five articles install:

    esword=0x6  fusionsword=0x7  parachute=0x8  staff=0x9  yato=0xa

Chrom's jab therefore shows the merged mesh rather than an article, which is the pre-article
behaviour and is expected here rather than a regression.

Two things are deliberately switched off and are NOT represented in this build's behaviour:

- **`ARTICLE_ROOT_ROTATION`** — Part 11 derived that a held article's root bone carries its owner's
  socket bind rotation, and that derivation is unaffected by anything since. It was gated off while
  chasing the stalls and never reached the card, so the Yato's "straight up in the body" in this
  build is the UNFIXED behaviour, not a failed fix.
- **`own_motion`** on the parachute — Part 6's native `change_motion` path, disabled after the
  build carrying it crashed. The crash was later traced elsewhere, so this is also untested rather
  than disproven.

## Known-good facts this build establishes

- `CLONE_ENABLED=false` loads and plays; with only `sword` disabled it also loads and plays. So the
  crash is that row, consistent with `exist=false` on 32 consecutive samples across two boots.
- The crash itself is the engine calling a virtual method on a null article during the DEMO/entry
  status, reached from `RemoveArticleByIndex`. See Part 12 of `docs/the-article-path.md`.
