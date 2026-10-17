# Contexts

This repo holds two bounded contexts. They share the game and almost nothing else.

| Context | Lives in | Glossary | Decisions |
|---|---|---|---|
| **The moveset** — one hand-built mod that puts 25 chosen moves on Pikachu | `pikachu-moveset/` | [CONTEXT.md](CONTEXT.md) | `pikachu-moveset/docs/adr/` |
| **The builder** — a tool that puts any fighter's move or stat on any other fighter | `mod-builder/` | [mod-builder/CONTEXT.md](mod-builder/CONTEXT.md) | `mod-builder/docs/adr/` |

`handbook/` is neither: it is the shared knowledge about the game itself, true in both contexts.

**Where the two glossaries deliberately disagree.** The root `CONTEXT.md` defines *host fighter* as
"…**Here: Pikachu**" and defines a *parity table* as the hand-edited record from which scripts are
generated. Both are correct for the moveset and wrong for the builder, where the host is a
parameter and nothing is hand-edited. The builder's glossary restates the first and replaces the
second with *requirements manifest*. Neither file is stale; they describe different contexts.
