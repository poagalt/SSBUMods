#!/usr/bin/env python3
"""One place to look anything up about the game, for modding: code, data, constants, names, docs.

    python tools/ssbu.py build                     # (re)build work/lookup/ssbu-lookup.sqlite
    python tools/ssbu.py find generate_rope        # EVERYTHING that knows this name, address or value
    python tools/ssbu.py find 0x200000ea           #   ... a work id, an address, a hash40
    python tools/ssbu.py hash special_s            # hash40 both ways
    python tools/ssbu.py const SPECIAL_S_GENERATED_ROPE   # lua constants by name or value
    python tools/ssbu.py doc "rope article"        # full-text search over every doc and tutorial
    python tools/ssbu.py sources                   # every source this draws on, with counts

    python tools/ssbu.py move pikachu attack_11    # = tools/move-index.py move ...
    python tools/ssbu.py code callers main 0xabe800   # = tools/code-index.py callers ...

WHAT IT JOINS. Three databases and a set of flat sources, each labelled with where a fact came from
so a reader can weigh it:

  the game's own      work/code-index/ssbu-code.sqlite (tools/code-index.sh): main and 96 fighter
                      NROs, fully analysed -- functions, symbols, code+data references, strings
                      work/move-index/ssbu-moves.sqlite (tools/move-index.py): motions, ACMD,
                      hitboxes, effects, sounds, articles, statuses, params, arc assets
                      vendor/main/lua-const-table.tsv: all 19,310 lua constants, names AND values
                      work/emu/consts-1305.csv: 39,213 constants recovered by emulating main
  dictionaries        hash40 names: vendor/Hashes.txt, ArcHashes.txt, param-labels, hash40-dict
  community           vendor/ssbu-decomp (sbergeron42): 11,087 named functions. Labelled 13.0.1,
                      but 511 of the 518 exports both it and this project's 13.0.5 dump name sit at
                      the same address, measured 2026-10-12; names are shown as "community"
  docs                every .md in handbook/, pikachu-moveset/docs/, mod-builder/, the backlog,
                      and the vendored tutorials and references (ssbu-mod-workshop, hdr, refs)

Precedence when they disagree is handbook/README.md's: the recording, then the game's own code,
then its data, then another mod, then a wiki. `find` prints the source on every line for that.
"""

import argparse
import csv
import glob
import os
import re
import sqlite3
import subprocess
import sys
import zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "work", "lookup")
DB = os.path.join(OUT, "ssbu-lookup.sqlite")
CODE_DB = os.path.join(ROOT, "work", "code-index", "ssbu-code.sqlite")
MOVE_DB = os.path.join(ROOT, "work", "move-index", "ssbu-moves.sqlite")
NRO_BASE = 0x7100000000

DOC_GLOBS = ["handbook/*.md", "pikachu-moveset/docs/*.md", "pikachu-moveset/docs/adr/*.md", "mod-builder/*.md",
             "mod-builder/docs/**/*.md", "*.md", "vendor/main/README-rat.md", "vendor/nro/README-rat.md",
             "vendor/ssbu-mod-workshop/**/*.md", "vendor/ssbu-decomp/docs/**/*.md", "vendor/ssbu-decomp/*.md",
             "vendor/hdr/**/*.rs", "vendor/hdr/*.md", "vendor/smashwiki/*", "vendor/refs/*/README*",
             "vendor/ssbuGeorgeMoveset/**/*.rs", "vendor/*.md"]


def hash40(s):
    b = s.encode()
    return (len(b) << 32) | zlib.crc32(b)


def log(m):
    print(m, flush=True)


# ---------------------------------------------------------------------------------------------

SCHEMA = """
create table sources (name text, kind text, path text, rows int, note text);
create table constants (name text, value int, source text, hash int, registered_by int);
create table hashes (hash int, name text, source text);
create table community (addr int, name text, size int, quality text, source text);
create virtual table docs using fts5(path, line unindexed, heading, text, tokenize='porter unicode61');
"""


def cmd_build(a):
    os.makedirs(OUT, exist_ok=True)
    tmp = DB + ".tmp"
    if os.path.exists(tmp):
        os.remove(tmp)
    con = sqlite3.connect(tmp)
    con.executescript("pragma journal_mode=off; pragma synchronous=off;" + SCHEMA)
    def src(name, kind, path, rows, note):
        if os.path.isabs(path):
            path = os.path.relpath(path, ROOT)
        con.execute("insert into sources values (?,?,?,?,?)", (name, kind, path.replace("\\", "/"), rows, note))

    # lua constants: the table main registers, with values, and the emulator's recovery
    n = 0
    p = os.path.join(ROOT, "vendor", "main", "lua-const-table.tsv")
    for line in open(p, encoding="utf-8"):
        if line.startswith("#"):
            continue
        f = line.rstrip("\n").split("\t")
        if len(f) >= 3:
            try:
                con.execute("insert into constants values (?,?,?,?,?)", (f[1], int(f[2], 16), "lua-const-table", None, None))
                n += 1
            except ValueError:
                pass
    src("lua constants", "game", p, n, "const_value_table__ of 13.0.5 main, name and value")
    n = 0
    p = os.path.join(ROOT, "work", "emu", "consts-1305.csv")
    if os.path.exists(p):
        rows = (r for r in csv.reader(l for l in open(p, encoding="utf-8") if not l.startswith("#")))
        for r in rows:
            if len(r) >= 4:
                try:
                    con.execute("insert into constants values (?,?,?,?,?)",
                                (r[2], int(r[3], 16), "emulated-registration", int(r[1], 16),
                                 int(r[4], 16) if len(r) > 4 and r[4] else None))
                    n += 1
                except ValueError:
                    pass
        src("emulated constants", "game", p, n, "tools/emu-module.py consts: what main registers, by emulation")

    # hash40 names
    seen = set()
    for p, label in (("vendor/Hashes.txt", "Hashes.txt"), ("vendor/ArcHashes.txt", "ArcHashes.txt")):
        n = 0
        for line in open(os.path.join(ROOT, p), encoding="utf-8", errors="replace"):
            s = line.strip()
            if s and s != "/":
                for name in {s, os.path.basename(s)}:
                    h = hash40(name)
                    if (h, name) not in seen:
                        seen.add((h, name)); con.execute("insert into hashes values (?,?,?)", (h, name, label)); n += 1
        src(label, "dictionary", p, n, "arc path components and file names")
    for p, label in (("vendor/param-labels/ParamLabels.csv", "ParamLabels"), ("vendor/main/hash40-dict.txt", "hash40-dict")):
        n = 0
        for line in open(os.path.join(ROOT, p), encoding="utf-8", errors="replace"):
            if line.startswith("#"):
                continue
            parts = re.split(r"[,\s]+", line.strip(), maxsplit=1)
            if len(parts) == 2 and parts[0].startswith("0x"):
                try:
                    h = int(parts[0], 16)
                except ValueError:
                    continue
                if (h, parts[1]) not in seen:
                    seen.add((h, parts[1])); con.execute("insert into hashes values (?,?,?)", (h, parts[1], label)); n += 1
        src(label, "dictionary", p, n, "param and lua names")

    # community names for main
    n = 0
    p = os.path.join(ROOT, "vendor", "ssbu-decomp", "data", "functions.csv")
    if os.path.exists(p):
        with open(p, newline="", encoding="utf-8") as f:
            for r in csv.DictReader(f):
                if r.get("Name", "").startswith("FUN_"):
                    continue
                try:
                    con.execute("insert into community values (?,?,?,?,?)",
                                (int(r["Address"], 16) - NRO_BASE, r["Name"], int(r["Size"] or 0), r.get("Quality"),
                                 "ssbu-decomp"))
                    n += 1
                except (ValueError, KeyError):
                    pass
        src("ssbu-decomp names", "community", p, n,
            "sbergeron42 matching decomp; 511/518 shared exports at identical addresses (2026-10-12)")

    # docs, by line, with the nearest heading
    files = sorted({f for g in DOC_GLOBS for f in glob.glob(os.path.join(ROOT, g), recursive=True) if os.path.isfile(f)})
    nlines = 0
    for f in files:
        rel = os.path.relpath(f, ROOT).replace("\\", "/")
        heading = ""
        try:
            text = open(f, encoding="utf-8", errors="replace").read().split("\n")
        except OSError:
            continue
        for i, line in enumerate(text, 1):
            if line.startswith("#"):
                heading = line.lstrip("# ").strip()
            if line.strip():
                con.execute("insert into docs values (?,?,?,?)", (rel, i, heading, line.strip()))
                nlines += 1
    src("docs", "docs", "handbook/, pikachu-moveset/docs/, mod-builder/, vendor references", nlines,
        f"{len(files)} files, searched by `doc`")

    for p, name, note in ((CODE_DB, "code index", "tools/code-index.sh"), (MOVE_DB, "move index", "tools/move-index.py")):
        if os.path.exists(p):
            c2 = sqlite3.connect(f"file:{p}?mode=ro", uri=True)
            if name == "code index":
                rows = c2.execute("select count(*) from functions").fetchone()[0]
                mods = c2.execute("select count(*) from modules").fetchone()[0]
                src(name, "game", os.path.relpath(p, ROOT), rows, f"{mods} modules; {note}")
            else:
                rows = c2.execute("select count(*) from motions").fetchone()[0]
                src(name, "game", os.path.relpath(p, ROOT), rows, f"c00 motions across 96 fighters; {note}")
            c2.close()
    con.executescript("create index c1 on constants(name); create index c2 on constants(value);"
                      "create index h1 on hashes(hash); create index h2 on hashes(name);"
                      "create index k1 on community(addr); create index k2 on community(name);")
    con.commit()
    con.close()
    os.replace(tmp, DB)
    log("wrote " + os.path.relpath(DB, ROOT))
    cmd_sources(a)


# ---------------------------------------------------------------------------------------------

class Lookup:
    def __init__(self):
        if not os.path.exists(DB):
            sys.exit(f"no {DB}; run `python tools/ssbu.py build`")
        self.con = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
        for alias, p in (("code", CODE_DB), ("moves", MOVE_DB)):
            if os.path.exists(p):
                self.con.execute(f"attach database 'file:{p.replace(os.sep, '/')}?mode=ro' as {alias}")
        dbs = [r[1] for r in self.con.execute("pragma database_list")]
        self.has_code, self.has_moves = "code" in dbs, "moves" in dbs

    def q(self, sql, *a):
        return self.con.execute(sql, a).fetchall()


def as_int(s):
    try:
        return int(s, 16) if s.lower().startswith("0x") else int(s)
    except ValueError:
        return None


def cmd_find(a):
    L = Lookup()
    t = a.term
    v = as_int(t)
    shown = 0

    def say(source, line):
        nonlocal shown
        print(f"  [{source}] {line}")
        shown += 1

    if v is not None:
        print(f"== {t} as a number")
        for name, src in L.q("select name, source from hashes where hash=? limit 10", v):
            say(f"hash40 {src}", name)
        for name, src in L.q("select name, source from constants where value=? limit 25", v):
            say(f"constant {src}", name)
        for name, src in L.q("select name, source from constants where hash=? limit 5", v):
            say("constant hash40", f"{name}  ({src})")
        if L.has_code:
            for mod in ("main",) if v < NRO_BASE else [r[0] for r in L.q("select module from code.modules where module!='main'")]:
                r = L.q("select entry, qname, bytes from code.functions where module=? and entry<=? and end>=? "
                        "order by entry desc limit 1", mod, v, v)
                if r:
                    e, qn, b = r[0]
                    say(f"code {mod}", f"in {qn} (0x{e:x}, {b} bytes)" + (f" +0x{v - e:x}" if v != e else ""))
                    for (s,) in L.q("select value from code.strings where module=? and addr=?", mod, v):
                        say(f"code {mod}", f"string {s[:100]!r}")
                    if mod != "main":
                        pass
            for name, src in L.q("select name, source from community where addr=?", v):
                say(f"community {src}", f"main 0x{v:x} = {name}")
    if not re.fullmatch(r"(0x)?[0-9a-fA-F]+", t) or v is None:
        print(f"== {t!r} as a name")
        h = hash40(t)
        hn = L.q("select source from hashes where hash=? and name=?", h, t)
        say("hash40", f"hash40({t}) = 0x{h:x}" + (f"  (known: {', '.join(s for (s,) in hn)})" if hn else ""))
        for name, val, src in L.q("select name, value, source from constants where name=? or name like ? "
                                  "order by name=? desc, name limit 15", t, f"%{t}%", t):
            say(f"constant {src}", f"{name} = 0x{val:x}")
        if L.has_code:
            for mod, addr, qn, ty in L.q("select module, addr, qname, type from code.symbols where name=? or qname like ? "
                                         "order by module='main' desc, module limit 20", t, f"%{t}%"):
                say(f"code {mod}", f"0x{addr:x}  {ty:<8} {qn}")
            for mod, addr, val in L.q("select module, addr, value from code.strings where value like ? limit 10", f"%{t}%"):
                say(f"code {mod}", f"string at 0x{addr:x}: {val[:90]!r}")
        for addr, name, src in L.q("select addr, name, source from community where name like ? limit 10", f"%{t}%"):
            say(f"community {src}", f"main 0x{addr:x}  {name}")
        if L.has_moves:
            for f, agent, kind, game in L.q("select fighter, agent, kind, game_script from moves.motions where kind=? "
                                            "or game_script=? limit 15", t, t):
                say("moves motion", f"{f}/{agent} {kind} -> {game}")
            for f, script, path, l0 in L.q("select fighter, script, motion_file, line_start from moves.acmd where script=? "
                                           "limit 10", t):
                say("moves acmd", f"{f} {script}  {path}:{l0}")
            for f, path, val in L.q("select fighter, path, value from moves.params where path like ? limit 10", f"%{t}%"):
                say("moves param", f"{f} {path} = {val}")
    print(f"== docs mentioning {t!r}")
    try:
        for path, line, heading, text in L.q("select path, line, heading, text from docs where docs match ? "
                                             "order by rank limit ?", '"' + t.replace('"', "") + '"', a.limit):
            say("doc", f"{path}:{line}  ({heading[:40]})  {text[:110]}")
    except sqlite3.OperationalError:
        pass
    if not shown:
        print("  nothing")


def cmd_hash(a):
    L = Lookup()
    v = as_int(a.what)
    if v is not None and a.what.lower().startswith("0x"):
        rows = L.q("select name, source from hashes where hash=?", v)
        print("\n".join(f"0x{v:x}  {n}  ({s})" for n, s in rows) or f"0x{v:x}  unknown")
    else:
        print(f"0x{hash40(a.what):x}  {a.what}")


def cmd_const(a):
    L = Lookup()
    v = as_int(a.what)
    rows = L.q("select name, value, source from constants where value=? limit 50", v) if v is not None else \
        L.q("select name, value, source from constants where name like ? order by name limit 50", f"%{a.what}%")
    for n, val, s in rows:
        print(f"0x{val:x}\t{n}\t({s})")


def cmd_doc(a):
    L = Lookup()
    for path, line, heading, text in L.q("select path, line, heading, text from docs where docs match ? order by rank "
                                         "limit ?", a.words, a.limit):
        print(f"{path}:{line}  [{heading[:50]}]\n    {text[:200]}")


def cmd_sources(a):
    L = Lookup()
    for name, kind, path, rows, note in L.q("select * from sources order by kind, name"):
        print(f"{kind:<10} {name:<22} {rows:>10,}  {path}\n{'':<34}{note}")


def passthrough(tool):
    def run(a):
        sys.exit(subprocess.call([sys.executable, os.path.join(ROOT, "tools", tool)] + a.rest))
    return run


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("build").set_defaults(f=cmd_build)
    p = sub.add_parser("find"); p.add_argument("term"); p.add_argument("--limit", type=int, default=12); p.set_defaults(f=cmd_find)
    p = sub.add_parser("hash"); p.add_argument("what"); p.set_defaults(f=cmd_hash)
    p = sub.add_parser("const"); p.add_argument("what"); p.set_defaults(f=cmd_const)
    p = sub.add_parser("doc"); p.add_argument("words"); p.add_argument("--limit", type=int, default=20); p.set_defaults(f=cmd_doc)
    sub.add_parser("sources").set_defaults(f=cmd_sources)
    for name, tool in (("move", "move-index.py"), ("code", "code-index.py")):
        p = sub.add_parser(name, add_help=False); p.add_argument("rest", nargs=argparse.REMAINDER)
        p.set_defaults(f=passthrough(tool))
    a = ap.parse_args()
    if a.cmd == "move":
        a.rest = ["move"] + a.rest if a.rest and a.rest[0] not in ("move", "moves", "missing", "sql", "build") else a.rest
    a.f(a)


if __name__ == "__main__":
    main()
