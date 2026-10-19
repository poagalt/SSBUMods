#!/usr/bin/env python3
"""Query the game's code without opening Ghidra: every function, name, reference and string.

    python tools/code-index.py build                       # load work/code-index/export/* into SQLite
    python tools/code-index.py fn main 0xabe800            # one function: row, then its C
    python tools/code-index.py fn pikachu SPECIAL_S__STATUS_MAIN
    python tools/code-index.py callers main generate_rope  # who calls it (direct calls)
    python tools/code-index.py refs-to main 0xabede0       # EVERYTHING that references it: code and data
    python tools/code-index.py refs-from main 0xabe800     # what a function reaches
    python tools/code-index.py strings main "rope"         # strings, and which functions use them
    python tools/code-index.py sym main Specializer_Gaogaen
    python tools/code-index.py sql "select count(*) from refs where module='main'"
    python tools/code-index.py asm packun 0x71000336a0     # one function's instructions (asm.txt.gz)

    python tools/code-index.py merge                       # after `code-index.sh pointers`: fold each
                                                           #   export.next/<m>/decomp-added.c into decomp.c
    python tools/code-index.py swap                        # export/<m> -> export.old/<m>, export.next/<m> -> export/<m>

WHY. `work/ghidra/main-all-100k.c` is text decompiled from `.text` alone: no strings, no data, so
nothing reached through a pointer has a caller in it. This index is built from a FULL Ghidra
analysis of `work/rea/*.elf` (tools/rea-elf.py), which carry `.rodata`/`.data` with the
module's own relocations applied, so a vtable slot, a spawn table or a status table is a
reference like any call. The analysis is paid once (tools/code-index.sh), not per question.

Modules are named as their ELF: `main`, `lua2cpp_pikachu`, ...; `pikachu` is accepted for
`lua2cpp_pikachu`. Addresses are the project's own: main from 0x0, NROs from 0x7100000000.
"""

import argparse
import bisect
import os
import shutil
import sqlite3
import sys
import zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BASE = os.path.join(ROOT, "work", "code-index")
EXPORT = os.path.join(BASE, "export")
NEXT = os.path.join(BASE, "export.next")   # what `code-index.sh pointers` writes; `swap` promotes it
OLD = os.path.join(BASE, "export.old")     # what `swap` moves aside
DB = os.path.join(BASE, "ssbu-code.sqlite")

SCHEMA = """
create table modules (module text primary key, functions int, symbols int, refs int, strings int,
                      decomp_path text, summary text, asm_path text);
create table functions (module text, entry int, end int, bytes int, name text, qname text,
                        thunk_target int, signature text, decomp_off int, decomp_len int,
                        asm_member int);
create table symbols (module text, addr int, name text, qname text, type text, source text,
                      is_primary int);
create table refs (module text, src int, dst int, type text, src_fn int);
create table strings (module text, addr int, length int, value text);
"""
INDEXES = """
create index fn_entry on functions(module, entry);
create index fn_name on functions(module, name);
create index sym_addr on symbols(module, addr);
create index sym_name on symbols(module, name);
create index ref_dst on refs(module, dst);
create index ref_srcfn on refs(module, src_fn);
create index str_addr on strings(module, addr);
"""


def h(x):
    return None if x in (None, "") else int(x, 16)


def unesc(s):
    if "\\" not in s:
        return s
    out, i = [], 0
    while i < len(s):
        c = s[i]
        if c == "\\" and i + 1 < len(s):
            n = s[i + 1]
            if n in "tnr\\":
                out.append({"t": "\t", "n": "\n", "r": "\r", "\\": "\\"}[n]); i += 2; continue
            if n == "x" and i + 3 < len(s):
                out.append(chr(int(s[i + 2:i + 4], 16))); i += 4; continue
        out.append(c); i += 1
    return "".join(out)


def rows(path):
    with open(path, encoding="utf-8", newline="\n") as f:
        next(f)
        for line in f:
            yield line.rstrip("\n").split("\t")


def decomp_offsets(path):
    """Byte offset and length of each `/* ==== 0x… ` section, keyed by entry."""
    offs, cur, start, pos = {}, None, 0, 0
    with open(path, "rb") as f:
        for line in f:
            if line.startswith(b"/* ==== 0x"):
                if cur is not None:
                    offs[cur] = (start, pos - start)
                cur, start = int(line[8:].split(None, 1)[0], 16), pos
            pos += len(line)
    if cur is not None:
        offs[cur] = (start, pos - start)
    return offs


def asm_members(d):
    """([first entry of each gzip member], [its byte offset]) from asm.idx, in address order."""
    firsts, offs = [], []
    for r in rows(os.path.join(d, "asm.idx")):
        firsts.append(h(r[0])); offs.append(int(r[1]))
    return firsts, offs


def asm_section(path, member_off, entry):
    """One function's lines out of asm.txt.gz: seek to its gzip member, inflate just that member."""
    with open(path, "rb") as f:
        f.seek(member_off)
        z, out = zlib.decompressobj(16 + zlib.MAX_WBITS), []
        while not z.eof:
            chunk = f.read(1 << 16)
            if not chunk:
                break
            out.append(z.decompress(chunk))
    text = b"".join(out).decode("utf-8", "replace")
    head = f"/* ==== 0x{entry:x}  "
    i = text.find(head)
    if i < 0:
        return None
    j = text.find("\n/* ==== 0x", i + len(head))
    return text[i:] if j < 0 else text[i:j + 1]


def sections(path):
    """(entry, bytes) for every `/* ==== 0x… ` section of a decomp file, in file order."""
    cur, buf = None, []
    with open(path, "rb") as f:
        for line in f:
            if line.startswith(b"/* ==== 0x"):
                if cur is not None:
                    yield cur, b"".join(buf)
                cur, buf = int(line[8:].split(None, 1)[0], 16), [line]
            else:
                buf.append(line)
    if cur is not None:
        yield cur, b"".join(buf)


def cmd_merge(a):
    """export.next/<m>/decomp.c = export/<m>/decomp.c with export.next/<m>/decomp-added.c folded in.

    Old sections are copied byte for byte; an added section goes in at its address, and one whose
    entry the old file already has (a function whose body changed) replaces it. Then the merged
    entries are checked against export.next/<m>/functions.tsv: every function must have one."""
    if not os.path.isdir(NEXT):
        sys.exit(f"no {NEXT}; run `bash tools/code-index.sh pointers ...` first")
    mods = [m for m in sorted(os.listdir(NEXT)) if a.module in (None, m, "lua2cpp_" + str(a.module))]
    bad = 0
    for m in mods:
        nd, od = os.path.join(NEXT, m), os.path.join(EXPORT, m)
        added_path = os.path.join(nd, "decomp-added.c")
        if not os.path.exists(added_path):
            print(f"{m}: no decomp-added.c, nothing to merge")
            continue
        added = dict(sections(added_path))
        pending = sorted(added)
        k = n_old = n_new = n_repl = 0
        tmp = os.path.join(nd, "decomp.c.tmp")
        with open(tmp, "wb") as w:
            for e, body in sections(os.path.join(od, "decomp.c")):
                while k < len(pending) and pending[k] < e:
                    w.write(added[pending[k]]); k += 1; n_new += 1
                if k < len(pending) and pending[k] == e:
                    w.write(added[e]); k += 1; n_repl += 1
                else:
                    w.write(body); n_old += 1
            while k < len(pending):
                w.write(added[pending[k]]); k += 1; n_new += 1
        want = {h(r[0]) for r in rows(os.path.join(nd, "functions.tsv"))}
        have = set(decomp_offsets(tmp))
        missing, extra = want - have, have - want
        os.replace(tmp, os.path.join(nd, "decomp.c"))
        ok = not missing
        bad += not ok
        print(f"{m}: {n_old:,} old sections kept, {n_new:,} added, {n_repl:,} replaced -> {len(have):,}; "
              f"functions.tsv {len(want):,}" + ("" if ok else f"; MISSING {len(missing)}") +
              (f"; {len(extra)} old sections with no function now" if extra else ""), flush=True)
    if bad:
        sys.exit(f"{bad} module(s) incomplete")


def cmd_swap(a):
    """Promote export.next/<m> to export/<m>, moving the old one to export.old/<m>."""
    if not os.path.isdir(NEXT):
        sys.exit(f"no {NEXT}")
    os.makedirs(OLD, exist_ok=True)
    for m in sorted(os.listdir(NEXT)):
        nd, od, bd = os.path.join(NEXT, m), os.path.join(EXPORT, m), os.path.join(OLD, m)
        if not os.path.isfile(os.path.join(nd, "decomp.c")):
            print(f"{m}: no merged decomp.c in export.next, left alone (run merge)")
            continue
        if os.path.exists(bd):
            sys.exit(f"{bd} already exists; move it away first")
        if os.path.exists(od):
            shutil.move(od, bd)
        shutil.move(nd, od)
        print(f"{m}: swapped (previous export in {os.path.relpath(bd, ROOT)})")


def cmd_build(a):
    os.makedirs(BASE, exist_ok=True)
    tmp = DB + ".tmp"
    if os.path.exists(tmp):
        os.remove(tmp)
    con = sqlite3.connect(tmp)
    con.executescript("pragma journal_mode=off; pragma synchronous=off;" + SCHEMA)
    mods = sorted(d for d in os.listdir(EXPORT) if os.path.isfile(os.path.join(EXPORT, d, "functions.tsv")))
    if not mods:
        sys.exit(f"nothing to load in {EXPORT}; run tools/code-index.sh first")
    for m in mods:
        d = os.path.join(EXPORT, m)
        dc = os.path.join(d, "decomp.c")
        offs = decomp_offsets(dc) if os.path.exists(dc) else {}
        ag = os.path.join(d, "asm.txt.gz")
        members = asm_members(d) if os.path.exists(ag) else None

        def member(e):
            if not members:
                return None
            i = bisect.bisect_right(members[0], e) - 1
            return members[1][i] if i >= 0 else None
        con.executemany("insert into functions values (?,?,?,?,?,?,?,?,?,?,?)",
                        ((m, h(r[0]), h(r[1]), int(r[2]), unesc(r[3]), unesc(r[4]), h(r[5]), unesc(r[6]),
                          *offs.get(h(r[0]), (None, None)), member(h(r[0])))
                         for r in rows(os.path.join(d, "functions.tsv"))))
        con.executemany("insert into symbols values (?,?,?,?,?,?,?)",
                        ((m, h(r[0]), unesc(r[1]), unesc(r[2]), r[3], r[4], int(r[5]))
                         for r in rows(os.path.join(d, "symbols.tsv"))))
        # References to addresses at or above 2^63 are not references: measured on main 2026-10-12,
        # 61,847 of 3.88M, 51,777 of them to 0xffffffffff... (-1 sentinels and other negative
        # words) and the rest to data that is not a pointer at all -- pairs of 1.0f (0x3f8000003f...)
        # and AArch64 instruction bytes (0x910203fda9...). Ghidra's analysis made references out of
        # them; they lie outside every loaded block and outside SQLite's signed 64-bit range, so
        # they are counted and dropped rather than stored.
        dropped = [0]

        def ref_rows():
            for r in rows(os.path.join(d, "refs.tsv")):
                vals = (h(r[0]), h(r[1]), h(r[3]))
                if any(v is not None and v >= 1 << 63 for v in vals):
                    dropped[0] += 1
                    continue
                yield (m, vals[0], vals[1], r[2], vals[2])
        con.executemany("insert into refs values (?,?,?,?,?)", ref_rows())
        if dropped[0]:
            print(f"  {m}: {dropped[0]} reference(s) to addresses beyond 2^63 dropped", flush=True)
        con.executemany("insert into strings values (?,?,?,?)",
                        ((m, h(r[0]), int(r[1]), unesc(r[2])) for r in rows(os.path.join(d, "strings.tsv"))))
        counts = [con.execute(f"select count(*) from {t} where module=?", (m,)).fetchone()[0]
                  for t in ("functions", "symbols", "refs", "strings")]
        summ = open(os.path.join(d, "summary.txt"), encoding="utf-8").read() if os.path.exists(
            os.path.join(d, "summary.txt")) else ""
        con.execute("insert into modules values (?,?,?,?,?,?,?,?)",
                    (m, *counts, os.path.relpath(dc, ROOT) if offs else None, summ,
                     os.path.relpath(ag, ROOT) if members else None))
        con.commit()
        print(f"{m}: {counts[0]:,} functions, {counts[1]:,} symbols, {counts[2]:,} refs, "
              f"{counts[3]:,} strings, {len(offs):,} decompiled", flush=True)
    con.executescript(INDEXES)
    con.close()
    os.replace(tmp, DB)
    print("wrote", os.path.relpath(DB, ROOT))


class Index:
    def __init__(self):
        if not os.path.exists(DB):
            sys.exit(f"no {DB}; run `python tools/code-index.py build`")
        self.con = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
        self.mods = [r[0] for r in self.con.execute("select module from modules")]

    def module(self, m):
        if m in self.mods:
            return m
        if "lua2cpp_" + m in self.mods:
            return "lua2cpp_" + m
        sys.exit(f"no module {m!r}; have: {', '.join(self.mods)}")

    def resolve(self, m, what):
        """An address, or a name: exact, then qualified, then substring (must be unique)."""
        try:
            return int(what, 16)
        except ValueError:
            pass
        q = self.con.execute
        # A thunk carries its target's name (a module calling its own export through the PLT), so
        # an exact name prefers the function that is not a thunk.
        for sql, arg in (("select entry from functions where module=? and (name=? or qname=?) and thunk_target is null",
                          (what, what)),
                         ("select entry from functions where module=? and (name=? or qname=?)", (what, what)),
                         ("select addr from symbols where module=? and (name=? or qname=?)", (what, what)),
                         ("select entry from functions where module=? and qname like ? and thunk_target is null",
                          (f"%{what}%",)),
                         ("select addr from symbols where module=? and name like ?", (f"%{what}%",))):
            hits = sorted({r[0] for r in q(sql, (m, *arg))})
            if len(hits) == 1:
                return hits[0]
            if len(hits) > 1:
                marks = ",".join("?" * min(len(hits), 20))
                names = q("select entry, qname from functions where module=? and entry in (" + marks + ")",
                          (m, *hits[:20])).fetchall()
                sys.exit(f"{what!r} is ambiguous ({len(hits)} matches):\n" +
                         "\n".join(f"  0x{a:x}  {n}" for a, n in names))
        sys.exit(f"nothing named {what!r} in {m}")

    def label(self, m, addr):
        """Function containing addr, else the nearest symbol at or below it."""
        r = self.con.execute("select entry, qname from functions where module=? and entry<=? and end>=? "
                             "order by entry desc limit 1", (m, addr, addr)).fetchone()
        if r:
            return f"{r[1]}+0x{addr - r[0]:x}" if addr != r[0] else r[1]
        r = self.con.execute("select addr, qname from symbols where module=? and addr<=? order by addr desc limit 1",
                             (m, addr)).fetchone()
        return f"{r[1]}+0x{addr - r[0]:x}" if r and addr - r[0] < 0x10000 else ""


def cmd_fn(a):
    ix = Index(); m = ix.module(a.module); e = ix.resolve(m, a.what)
    r = ix.con.execute("select entry, end, bytes, qname, signature, decomp_off, decomp_len from functions "
                       "where module=? and entry<=? and end>=? order by entry desc limit 1", (m, e, e)).fetchone()
    if not r:
        sys.exit(f"0x{e:x} is not inside a function in {m}")
    print(f"0x{r[0]:x}..0x{r[1]:x}  {r[2]} bytes  {r[3]}\n  {r[4]}")
    if r[5] is not None and not a.no_c:
        path = ix.con.execute("select decomp_path from modules where module=?", (m,)).fetchone()[0]
        with open(os.path.join(ROOT, path), "rb") as f:
            f.seek(r[5]); print(f.read(r[6]).decode("utf-8", "replace"))


def cmd_asm(a):
    ix = Index(); m = ix.module(a.module); e = ix.resolve(m, a.what)
    r = ix.con.execute("select entry, asm_member from functions where module=? and entry<=? and end>=? "
                       "order by entry desc limit 1", (m, e, e)).fetchone()
    if not r:
        sys.exit(f"0x{e:x} is not inside a function in {m}")
    path = ix.con.execute("select asm_path from modules where module=?", (m,)).fetchone()[0]
    if path is None or r[1] is None:
        sys.exit(f"{m} has no asm.txt.gz in the index; re-export it (tools/code-index.sh pointers)")
    s = asm_section(os.path.join(ROOT, path), r[1], r[0])
    print(s if s is not None else f"0x{r[0]:x} not found in its asm member")


def cmd_callers(a):
    ix = Index(); m = ix.module(a.module); e = ix.resolve(m, a.what)
    for src, t, fn in ix.con.execute("select src, type, src_fn from refs where module=? and dst=? and "
                                     "type like '%CALL%' order by src", (m, e)):
        print(f"0x{src:x}  {t:<20} {ix.label(m, src)}")


def cmd_refs_to(a):
    ix = Index(); m = ix.module(a.module); e = ix.resolve(m, a.what)
    for src, t, fn in ix.con.execute("select src, type, src_fn from refs where module=? and dst=? order by src", (m, e)):
        where = ix.label(m, src) if fn is not None else "(data) " + ix.label(m, src)
        print(f"0x{src:x}  {t:<20} {where}")


def cmd_refs_from(a):
    ix = Index(); m = ix.module(a.module); e = ix.resolve(m, a.what)
    fe = ix.con.execute("select entry from functions where module=? and entry<=? and end>=? order by entry desc limit 1",
                        (m, e, e)).fetchone()
    if not fe:
        sys.exit(f"0x{e:x} is not inside a function")
    for src, dst, t in ix.con.execute("select src, dst, type from refs where module=? and src_fn=? order by src",
                                      (m, fe[0])):
        s = ix.con.execute("select value from strings where module=? and addr=?", (m, dst)).fetchone()
        print(f"0x{src:x} -> 0x{dst:x}  {t:<20} {repr(s[0])[:80] if s else ix.label(m, dst)}")


def cmd_strings(a):
    ix = Index(); m = ix.module(a.module)
    for addr, val in ix.con.execute("select addr, value from strings where module=? and value like ? order by addr",
                                    (m, f"%{a.text}%")):
        users = sorted({ix.label(m, r[0]) for r in ix.con.execute(
            "select src from refs where module=? and dst=?", (m, addr))})
        print(f"0x{addr:x}  {val[:100]!r}")
        for u in users[:12]:
            print(f"      used by {u}")


def cmd_sym(a):
    ix = Index(); m = ix.module(a.module)
    for addr, qn, t in ix.con.execute("select addr, qname, type from symbols where module=? and (name like ? or "
                                      "qname like ?) order by addr limit ?", (m, f"%{a.text}%", f"%{a.text}%", a.limit)):
        print(f"0x{addr:x}  {t:<9} {qn}")


def cmd_sql(a):
    ix = Index()
    for r in ix.con.execute(a.query):
        print("\t".join("" if v is None else str(v) for v in r))


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("build").set_defaults(f=cmd_build)
    p = sub.add_parser("merge"); p.add_argument("module", nargs="?"); p.set_defaults(f=cmd_merge)
    sub.add_parser("swap").set_defaults(f=cmd_swap)
    for name, f in (("fn", cmd_fn), ("asm", cmd_asm), ("callers", cmd_callers), ("refs-to", cmd_refs_to),
                    ("refs-from", cmd_refs_from)):
        p = sub.add_parser(name); p.add_argument("module"); p.add_argument("what"); p.set_defaults(f=f)
        if name == "fn":
            p.add_argument("--no-c", action="store_true", help="the function row only")
    for name, f in (("strings", cmd_strings), ("sym", cmd_sym)):
        p = sub.add_parser(name); p.add_argument("module"); p.add_argument("text"); p.set_defaults(f=f)
        p.add_argument("--limit", type=int, default=200)
    p = sub.add_parser("sql"); p.add_argument("query"); p.set_defaults(f=cmd_sql)
    a = ap.parse_args()
    a.f(a)


if __name__ == "__main__":
    main()
