#!/usr/bin/env python3
"""Every fighter's moves, joined from the game's own data: one place to read a move to port it.

    python tools/move-index.py build                    # ~minutes; writes work/move-index/
    python tools/move-index.py move pikachu attack_11   # one move: animation, frames, hitboxes,
                                                        #   effects (with colours), sounds, articles,
                                                        #   statuses that play it, and what is missing
    python tools/move-index.py moves gaogaen            # the fighter's motion list with ACMD coverage
    python tools/move-index.py missing pikachu          # assets the game has that this PC does not
    python tools/move-index.py sql "select ..."

Sheets for every fighter are written to work/move-index/sheets/<fighter>.md as part of `build`.

WHAT IT JOINS, AND FROM WHERE (all local; nothing here needs the console):

  motion lists   extracted/fighter/<f>/motion/<agent>/c00/motion_list.bin, read by
                 tools/motion-patch's `mljson` (motion_list_rs) -- kinds, animations, script
                 hashes, cancel frame, intangibility window, flags
  ACMD           vendor/SSBU-Dumped-Scripts/smashline/lua2cpp_<f>/<agent>/<Motion>.txt -- game_,
                 effect_, sound_, expression_ per motion, parsed into timed events. Linked to a
                 motion by HASH: hash40("game_attack11") is the motion's own game_script field
  animations     the .nuanmb each motion names; frame count from its own header (f32 at 0x18)
  statuses       work/rea/<f>-statusmap.txt (disasm --statusmap: kind, function, address in the
                 NRO) and vendor/ssbu-status-all (status C); a status "plays" a motion when its C
                 contains that motion kind's hash40 as a constant
  params         extracted/fighter/<f>/param/vl.prc via tools/prc-tool `prcdump`, and the
                 fighter's row of fighter/common/param/fighter_param.prc
  assets         work/acquire/arc-paths.txt (the arc's own file list, 590,711 paths) against
                 extracted/, work/arc-vis/, work/arc-weapons/ and work/arc-nro/

FRAMES. ACMD's `frame(N)` sets the cursor and `wait(N)` advances it, in SCRIPT frames. A hitbox
runs from its ATTACK until AttackModule::clear_all, a clear() of its id, or a new ATTACK with the
same id; `until` is null when the script never removes it (the status does). FT_MOTION_RATE
changes how script frames map to game frames and is recorded as an event, not applied: the
mapping is exactly what a port must reproduce, so it is shown, not folded away. Calls inside a
condition or loop carry `cond`/`loop` so a reader knows the timing is not unconditional.
"""

import argparse
import json
import os
import re
import sqlite3
import struct
import subprocess
import sys
import zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "work", "move-index")
DB = os.path.join(OUT, "ssbu-moves.sqlite")
ACMD = os.path.join(ROOT, "vendor", "SSBU-Dumped-Scripts", "smashline")
STATUS_C = os.path.join(ROOT, "vendor", "ssbu-status-all", "ghidra_scripts")
EXTRACTED = os.path.join(ROOT, "extracted")
LOCAL_ROOTS = [EXTRACTED, os.path.join(ROOT, "work", "arc-vis"), os.path.join(ROOT, "work", "arc-weapons"),
               os.path.join(ROOT, "work", "arc-nro"), os.path.join(ROOT, "vendor", "nro")]
MLJSON = os.path.join(ROOT, "tools", "motion-patch", "target", "release", "mljson.exe")
PRCDUMP = os.path.join(ROOT, "tools", "prc-tool", "target", "release", "prcdump.exe")
LABELS = os.path.join(ROOT, "vendor", "param-labels", "ParamLabels.csv")

# smashline macros::ATTACK, after `agent` (36 arguments; the same layout tools/reach acmdparity checks)
ATTACK_FIELDS = ["id", "part", "bone", "damage", "angle", "kbg", "fkb", "bkb", "size", "x", "y", "z",
                 "x2", "y2", "z2", "hitlag", "sdi", "setoff", "lr_check", "set_weight", "shield_damage",
                 "trip", "rehit", "reflectable", "absorbable", "flinchless", "disable_hitlag",
                 "direct", "situation_mask", "category_mask", "part_mask", "friendly_fire", "attr",
                 "sound_level", "sound_attr", "region"]
COLOR_CALLS = ("LAST_EFFECT_SET_COLOR", "LAST_PARTICLE_SET_COLOR", "EFFECT_FOLLOW_COLOR", "FLASH",
               "FLASH_FRM", "COL_PRI", "BURN_COLOR", "BURN_COLOR_FRAME", "LAST_EFFECT_SET_ALPHA")


def hash40(s):
    b = s.encode()
    return (len(b) << 32) | zlib.crc32(b)


def log(m):
    print(m, flush=True)


# ---------------------------------------------------------------------------------------------
# names for hashes
# ---------------------------------------------------------------------------------------------

class Names:
    def __init__(self):
        self.by_hash = {}

    def add(self, name):
        if name and name not in ("/",):
            self.by_hash.setdefault(hash40(name), name)

    def load(self):
        for p in ("vendor/Hashes.txt", "vendor/ArcHashes.txt"):
            for line in open(os.path.join(ROOT, p), encoding="utf-8", errors="replace"):
                n = line.strip()
                self.add(n)
                self.add(os.path.basename(n))
        for p in (LABELS, os.path.join(ROOT, "vendor", "main", "hash40-dict.txt")):
            for line in open(p, encoding="utf-8", errors="replace"):
                if line.startswith("#"):
                    continue
                parts = re.split(r"[,\s]+", line.strip(), maxsplit=1)
                if len(parts) == 2 and parts[0].startswith("0x"):
                    try:
                        self.by_hash.setdefault(int(parts[0], 16), parts[1].strip())
                    except ValueError:
                        pass
        return self

    def __call__(self, h):
        return self.by_hash.get(h)


# ---------------------------------------------------------------------------------------------
# ACMD
# ---------------------------------------------------------------------------------------------

FN_RE = re.compile(r'^unsafe extern "C" fn (\w+)\(')
CALL_RE = re.compile(r'^\s*(?:let\s+\w+\s*=\s*)?((?:macros::)?[A-Za-z_][\w]*(?:::[A-Za-z_]\w*)*!?)\((.*)\);\s*$')
FRAME_RE = re.compile(r'^\s*frame\(agent\.lua_state_agent,\s*([-\d.]+)\);')
WAIT_RE = re.compile(r'^\s*wait\(agent\.lua_state_agent,\s*([-\d.]+)\);')
HASH_RE = re.compile(r'Hash40::new\("([^"]*)"\)')


def split_args(s):
    out, depth, cur, q = [], 0, [], False
    for c in s:
        if c == '"':
            q = not q
        if not q:
            if c in "([{":
                depth += 1
            elif c in ")]}":
                depth -= 1
            elif c == "," and depth == 0:
                out.append("".join(cur).strip()); cur = []
                continue
        cur.append(c)
    if cur:
        out.append("".join(cur).strip())
    return out


def num(x):
    try:
        return float(x)
    except (TypeError, ValueError):
        return None


def parse_acmd_file(path):
    """Yield (function_name, line_start, line_end, events) for each function in a dump file."""
    lines = open(path, encoding="utf-8", errors="replace").read().split("\n")
    i = 0
    while i < len(lines):
        m = FN_RE.match(lines[i])
        if not m:
            i += 1
            continue
        name, start = m.group(1), i + 1
        events, frame, stack, seq = [], 0.0, [], 0
        depth = lines[i].count("{") - lines[i].count("}")
        i += 1
        while i < len(lines) and depth > 0:
            line = lines[i]
            st = line.strip()
            fm, wm = FRAME_RE.match(line), WAIT_RE.match(line)
            if fm:
                frame = float(fm.group(1))
            elif wm:
                frame += float(wm.group(1))
            elif st.startswith("if ") or st.startswith("} else") or st.startswith("else") \
                    or st.startswith("for ") or st.startswith("while ") or st.startswith("loop"):
                pass
            else:
                cm = CALL_RE.match(line)
                if cm and not cm.group(1).startswith(("frame", "wait")):
                    cond = any(k == "cond" for k in stack)
                    loop = any(k == "loop" for k in stack)
                    events.append(dict(seq=seq, frame=frame, call=cm.group(1).replace("macros::", ""),
                                       args=cm.group(2), cond=int(cond), loop=int(loop)))
                    seq += 1
            # block bookkeeping, after the line's own meaning is taken
            opens, closes = line.count("{"), line.count("}")
            for _ in range(closes):
                if stack:
                    stack.pop()
                depth -= 1
            if opens:
                if st.startswith("if macros::is_excute"):
                    kind = "exec"
                elif st.startswith(("for ", "while ", "loop")):
                    kind = "loop"
                elif st.startswith(("if ", "} else", "else", "match ")):
                    kind = "cond"
                else:
                    kind = "block"
                for _ in range(opens):
                    stack.append(kind)
                    depth += 1
            i += 1
        yield name, start, i, events


def hitboxes_from(events):
    """ATTACK rows with their active window."""
    out = []
    live = {}
    for e in events:
        c = e["call"]
        if c == "ATTACK":
            a = split_args(e["args"])[1:]
            if len(a) < 36:
                continue
            row = dict(zip(ATTACK_FIELDS, a[:36]))
            row["bone"] = (HASH_RE.findall(row["bone"]) or [row["bone"]])[0]
            row["attr"] = (HASH_RE.findall(row["attr"]) or [row["attr"]])[0]
            row.update(frame=e["frame"], until=None, cond=e["cond"], loop=e["loop"], seq=e["seq"])
            key = row["id"]
            if key in live and live[key]["until"] is None:
                live[key]["until"] = e["frame"]
            live[key] = row
            out.append(row)
        elif c == "AttackModule::clear_all":
            for r in live.values():
                if r["until"] is None:
                    r["until"] = e["frame"]
            live = {}
        elif c == "AttackModule::clear":
            a = split_args(e["args"])
            if len(a) >= 2 and a[1] in live and live[a[1]]["until"] is None:
                live[a[1]]["until"] = e["frame"]
                del live[a[1]]
    return out


def effects_from(events):
    out, last = [], None
    for e in events:
        c = e["call"]
        name = c.split("::")[-1]
        if name in COLOR_CALLS:
            a = split_args(e["args"])[1:]
            color = ",".join(a)
            if name.startswith("LAST_") and last is not None and not last.get("color"):
                last["color"] = f"{name}({color})"
            else:
                out.append(dict(frame=e["frame"], call=name, effect=None, bone=None, color=color,
                                args=e["args"], cond=e["cond"], loop=e["loop"]))
        elif "EFFECT" in name or name.startswith(("AFTER_IMAGE", "LANDING_", "FOOT_")):
            hs = HASH_RE.findall(e["args"])
            row = dict(frame=e["frame"], call=name, effect=hs[0] if hs else None,
                       bone=hs[1] if len(hs) > 1 else None, color=None, args=e["args"],
                       cond=e["cond"], loop=e["loop"])
            out.append(row)
            last = row
    return out


def sounds_from(events):
    out = []
    for e in events:
        name = e["call"].split("::")[-1]
        if name.startswith("PLAY_") or "SoundModule::" in e["call"]:
            hs = HASH_RE.findall(e["args"])
            out.append(dict(frame=e["frame"], call=name, sound=hs[0] if hs else None, cond=e["cond"]))
    return out


ARTICLE_RE = re.compile(r"\*(\w+_GENERATE_ARTICLE_\w+)")


def articles_from(events):
    out = []
    for e in events:
        if e["call"].startswith("ArticleModule::"):
            m = ARTICLE_RE.search(e["args"])
            out.append(dict(frame=e["frame"], call=e["call"].split("::")[-1],
                            article=m.group(1) if m else None, cond=e["cond"]))
    return out


# ---------------------------------------------------------------------------------------------
# other sources
# ---------------------------------------------------------------------------------------------

def nuanmb_frames(path):
    try:
        with open(path, "rb") as f:
            b = f.read(0x1C)
        if b[:4] == b"HBSS" and b[0x10:0x14] == b"MINA":
            return struct.unpack_from("<f", b, 0x18)[0] + 1.0
    except OSError:
        pass
    return None


def prc_rows(path):
    """prcdump's indented tree as (dotted.path, value)."""
    r = subprocess.run([PRCDUMP, path, LABELS, "--depth", "40"], capture_output=True, text=True,
                       encoding="utf-8", errors="replace")
    if r.returncode != 0:
        return []
    rows, stack = [], []
    for line in r.stdout.split("\n")[1:]:
        if not line.strip():
            continue
        ind = (len(line) - len(line.lstrip())) // 2 - 1
        body = line.strip()
        stack = stack[:ind]
        if " = " in body:
            k, v = body.split(" = ", 1)
            rows.append((".".join(stack + [k]), v))
        else:
            stack.append(body.split(" ")[0])
    return rows


def local_files():
    """Arc-relative path -> local path, over every root the project pulls into."""
    files = {}
    for root in LOCAL_ROOTS:
        if not os.path.isdir(root):
            continue
        for d, _, fs in os.walk(root):
            for f in fs:
                p = os.path.join(d, f)
                rel = os.path.relpath(p, root).replace("\\", "/")
                files.setdefault(rel, p)
    return files


def asset_category(path):
    parts = path.split("/")
    if parts[0] == "fighter" and len(parts) > 2:
        return parts[2] if parts[2] in ("model", "motion", "param") else "other"
    if parts[0] == "effect":
        return "effect"
    if parts[0] in ("sound", "stream:"):
        return "sound"
    if parts[0] == "prebuilt:":
        return "code"
    return "other"


def fighter_of_asset(path, fighters):
    parts = path.split("/")
    if parts[0] == "fighter" and len(parts) > 1:
        return parts[1]
    if parts[0] == "effect" and len(parts) > 2 and parts[1] == "fighter":
        return parts[2]
    base = os.path.basename(path)
    m = re.match(r"(?:se|vc|lua2cpp)_([a-z0-9]+)", base)
    if m and m.group(1) in fighters:
        return m.group(1)
    return None


# ---------------------------------------------------------------------------------------------
# build
# ---------------------------------------------------------------------------------------------

SCHEMA = """
create table fighters (fighter text primary key, acmd_agents int, acmd_scripts int, motions int,
                       statuses int, params int, assets int, assets_local int);
create table motions (fighter text, agent text, kind text, kind_hash int, game_script text,
                      scripts text, flags int, blend_frames int, cancel_frame int, xlu_start int,
                      xlu_end int, animation text, anim_local text, anim_frames real, acmd_agent text);
create table acmd (fighter text, agent text, script text, script_hash int, category text, motion_file text,
                   line_start int, line_end int, events int);
create table events (fighter text, agent text, script text, seq int, frame real, call text, args text,
                     cond int, loop int);
create table hitboxes (fighter text, agent text, script text, frame real, until real, cond int, loop int,
                       %s);
create table effects (fighter text, agent text, script text, frame real, call text, effect text, bone text,
                      color text, args text, cond int, loop int);
create table sounds (fighter text, agent text, script text, frame real, call text, sound text, cond int);
create table articles (fighter text, agent text, script text, frame real, call text, article text, cond int);
create table statuses (fighter text, kind text, func text, address int);
create table status_motions (fighter text, agent text, status text, motion text);
create table params (fighter text, source text, path text, value text);
create table assets (fighter text, category text, arc_path text, local_path text);
""" % ", ".join(f"{f} text" for f in ATTACK_FIELDS)
INDEXES = """
create index m1 on motions(fighter, kind); create index m2 on motions(fighter, game_script);
create index a1 on acmd(fighter, script); create index e1 on events(fighter, script);
create index h1 on hitboxes(fighter, script); create index f1 on effects(fighter, script);
create index s1 on sounds(fighter, script); create index r1 on articles(fighter, script);
create index p1 on params(fighter, path); create index x1 on assets(fighter, category);
create index sm on status_motions(fighter, motion);
"""


def cmd_build(a):
    os.makedirs(OUT, exist_ok=True)
    names = Names().load()
    fighters = sorted(d[len("lua2cpp_"):] for d in os.listdir(ACMD) if d.startswith("lua2cpp_"))
    log(f"{len(fighters)} fighters with ACMD dumps; {len(names.by_hash):,} known hash names")

    tmp = DB + ".tmp"
    if os.path.exists(tmp):
        os.remove(tmp)
    con = sqlite3.connect(tmp)
    con.executescript("pragma journal_mode=off; pragma synchronous=off;" + SCHEMA)

    # ---- ACMD: every script, parsed ------------------------------------------------------
    acmd_by_hash = {}   # (fighter, script_hash) -> script name
    for f in fighters:
        fdir = os.path.join(ACMD, f"lua2cpp_{f}")
        for agent in sorted(os.listdir(fdir)):
            adir = os.path.join(fdir, agent)
            if not os.path.isdir(adir):
                continue
            for mf in sorted(os.listdir(adir)):
                if not mf.endswith(".txt"):
                    continue
                path = os.path.join(adir, mf)
                for script, l0, l1, ev in parse_acmd_file(path):
                    names.add(script)
                    sh = hash40(script)
                    acmd_by_hash[(f, sh)] = script
                    cat = script.split("_", 1)[0]
                    con.execute("insert into acmd values (?,?,?,?,?,?,?,?,?)",
                                (f, agent, script, sh, cat, os.path.relpath(path, ROOT).replace("\\", "/"),
                                 l0, l1, len(ev)))
                    con.executemany("insert into events values (?,?,?,?,?,?,?,?,?)",
                                    ((f, agent, script, e["seq"], e["frame"], e["call"], e["args"],
                                      e["cond"], e["loop"]) for e in ev))
                    for h in hitboxes_from(ev):
                        con.execute("insert into hitboxes values (%s)" % ",".join("?" * (7 + len(ATTACK_FIELDS))),
                                    (f, agent, script, h["frame"], h["until"], h["cond"], h["loop"],
                                     *[h[k] for k in ATTACK_FIELDS]))
                    con.executemany("insert into effects values (?,?,?,?,?,?,?,?,?,?,?)",
                                    ((f, agent, script, x["frame"], x["call"], x["effect"], x["bone"], x["color"],
                                      x["args"], x["cond"], x["loop"]) for x in effects_from(ev)))
                    con.executemany("insert into sounds values (?,?,?,?,?,?,?)",
                                    ((f, agent, script, x["frame"], x["call"], x["sound"], x["cond"])
                                     for x in sounds_from(ev)))
                    con.executemany("insert into articles values (?,?,?,?,?,?,?)",
                                    ((f, agent, script, x["frame"], x["call"], x["article"], x["cond"])
                                     for x in articles_from(ev)))
        con.commit()
    log(f"ACMD: {con.execute('select count(*) from acmd').fetchone()[0]:,} scripts, "
        f"{con.execute('select count(*) from hitboxes').fetchone()[0]:,} hitboxes, "
        f"{con.execute('select count(*) from effects').fetchone()[0]:,} effect calls")

    # ---- motion lists (c00) ----------------------------------------------------------------
    cache = os.path.join(OUT, "motions.jsonl")
    if not os.path.exists(cache) or os.path.getmtime(cache) < os.path.getmtime(MLJSON):
        with open(cache, "w", encoding="utf-8") as fo:
            subprocess.run([MLJSON, os.path.join(EXTRACTED, "fighter")], stdout=fo, check=True)
    nm = 0
    motion_hashes = {}
    for line in open(cache, encoding="utf-8"):
        r = json.loads(line)
        parts = r["file"].replace("\\", "/").split("/")
        try:
            i = parts.index("fighter")
            f, agent, costume = parts[i + 1], parts[i + 3], parts[i + 4]
        except (ValueError, IndexError):
            continue
        if costume != "c00":
            continue
        kind = names(r["kind"]) or f"0x{r['kind']:x}"
        motion_hashes.setdefault((f, agent), {})[r["kind"]] = kind
        anim_names = [names(h) or f"0x{h:x}" for h, _ in r["animations"]]
        anim = anim_names[0] if anim_names else None
        rel = f"fighter/{f}/motion/{agent}/c00/{anim}" if anim and not anim.startswith("0x") else None
        loc = next((os.path.join(root, rel) for root in LOCAL_ROOTS if rel and os.path.exists(os.path.join(root, rel))), None)
        ex = r["extra"] or {}
        acmd_agent = f if agent == "body" else f"{f}_{agent}"
        con.execute("insert into motions values (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                    (f, agent, kind, r["kind"],
                     acmd_by_hash.get((f, r["game_script"])) or names(r["game_script"]) or f"0x{r['game_script']:x}",
                     json.dumps([acmd_by_hash.get((f, h)) or names(h) or f"0x{h:x}" for h in r["scripts"]]),
                     r["flags"], r["blend_frames"], ex.get("cancel_frame"), ex.get("xlu_start"), ex.get("xlu_end"),
                     json.dumps(anim_names), os.path.relpath(loc, ROOT).replace("\\", "/") if loc else None,
                     nuanmb_frames(loc) if loc else None, acmd_agent))
        nm += 1
    con.commit()
    log(f"motions: {nm:,} (c00)")

    # ---- statuses ---------------------------------------------------------------------------
    for f in fighters:
        for p in (os.path.join(ROOT, "vendor", "main", f"{f}-statusmap.txt"), os.path.join(ROOT, "work", "rea", f"{f}-statusmap.txt")):
            if os.path.exists(p):
                for line in open(p):
                    s = line.split()
                    if len(s) >= 3 and s[0].startswith("0x"):
                        con.execute("insert into statuses values (?,?,?,?)",
                                    (f, s[1], s[2].replace("LUA_SCRIPT_STATUS_FUNC_", ""), int(s[0], 16)))
                break
        sdir = os.path.join(STATUS_C, f"lua2cpp_{f}.nro", f)
        if os.path.isdir(sdir):
            for agent in os.listdir(sdir):
                st = os.path.join(sdir, agent, "status")
                if not os.path.isdir(st):
                    continue
                mh = motion_hashes.get((f, "body" if agent == f else agent[len(f) + 1:]), {})
                for status in os.listdir(st):
                    found = set()
                    for cf in os.listdir(os.path.join(st, status)):
                        txt = open(os.path.join(st, status, cf), encoding="utf-8", errors="replace").read()
                        for hx in re.findall(r"0x([0-9a-fA-F]{9,10})\b", txt):
                            h = int(hx, 16)
                            if h in mh:
                                found.add(mh[h])
                    con.executemany("insert into status_motions values (?,?,?,?)",
                                    ((f, agent, status, k) for k in sorted(found)))
    con.commit()
    log(f"statuses: {con.execute('select count(*) from statuses').fetchone()[0]:,} registered functions, "
        f"{con.execute('select count(*) from status_motions').fetchone()[0]:,} status->motion links")

    # ---- params -----------------------------------------------------------------------------
    for f in fighters:
        vl = os.path.join(EXTRACTED, "fighter", f, "param", "vl.prc")
        if os.path.exists(vl):
            con.executemany("insert into params values (?,?,?,?)", ((f, "vl.prc", k, v) for k, v in prc_rows(vl)))
    common = os.path.join(EXTRACTED, "fighter", "common", "param", "fighter_param.prc")
    if os.path.exists(common):
        rows = prc_rows(common)
        # fighter_param_table.[N].fighter_kind = fighter_kind_<f>: give each fighter its own row
        kinds = {}
        for k, v in rows:
            m = re.match(r"(fighter_param_table\.\[\d+\])\.fighter_kind$", k)
            if m and v.startswith("fighter_kind_"):
                kinds[m.group(1)] = v[len("fighter_kind_"):]
        con.executemany("insert into params values (?,?,?,?)",
                        ((kinds[k.split(".")[0] + "." + k.split(".")[1]], "fighter_param.prc",
                          ".".join(k.split(".")[2:]), v)
                         for k, v in rows if k.count(".") >= 2 and ".".join(k.split(".")[:2]) in kinds))
    con.commit()
    log(f"params: {con.execute('select count(*) from params').fetchone()[0]:,} values")

    # ---- assets: what the game has, against what this PC has ------------------------------
    loc = local_files()
    fset = set(fighters)
    batch = []
    for line in open(os.path.join(ROOT, "work", "acquire", "arc-paths.txt"), encoding="utf-8", errors="replace"):
        p = line.strip()
        if not p:
            continue
        f = fighter_of_asset(p, fset)
        if f is None:
            continue
        lp = loc.get(p) or loc.get(p.replace("prebuilt:/", "prebuilt/"))
        batch.append((f, asset_category(p), p, os.path.relpath(lp, ROOT).replace("\\", "/") if lp else None))
    con.executemany("insert into assets values (?,?,?,?)", batch)
    con.commit()
    log(f"assets: {len(batch):,} arc paths, {sum(1 for b in batch if b[3]):,} local")

    for f in fighters:
        q = lambda s, *a: con.execute(s, (f, *a)).fetchone()[0]
        con.execute("insert into fighters values (?,?,?,?,?,?,?,?)",
                    (f, q("select count(distinct agent) from acmd where fighter=?"),
                     q("select count(*) from acmd where fighter=?"), q("select count(*) from motions where fighter=?"),
                     q("select count(*) from statuses where fighter=?"), q("select count(*) from params where fighter=?"),
                     q("select count(*) from assets where fighter=?"),
                     q("select count(*) from assets where fighter=? and local_path is not null")))
    con.executescript(INDEXES)
    con.commit()
    con.close()
    os.replace(tmp, DB)
    log("wrote " + os.path.relpath(DB, ROOT))

    os.makedirs(os.path.join(OUT, "sheets"), exist_ok=True)
    ix = Index()
    for f in fighters:
        with open(os.path.join(OUT, "sheets", f"{f}.md"), "w", encoding="utf-8") as fo:
            fo.write(ix.sheet(f))
    log(f"wrote {len(fighters)} sheets to work/move-index/sheets/")


# ---------------------------------------------------------------------------------------------
# queries
# ---------------------------------------------------------------------------------------------

def fmt(x):
    if x is None:
        return ""
    if isinstance(x, float):
        return f"{x:g}"
    return str(x)


class Index:
    def __init__(self):
        if not os.path.exists(DB):
            sys.exit(f"no {DB}; run `python tools/move-index.py build`")
        self.con = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)

    def q(self, sql, *a):
        return self.con.execute(sql, a).fetchall()

    def motion_block(self, f, row):
        agent, kind, game, scripts, cancel, x0, x1, anims, aloc, frames, aagent, flags = row
        out = [f"### `{kind}`" + (f"  ({agent})" if agent != "body" else "")]
        anim = json.loads(anims)
        out.append(f"- animation: `{anim[0] if anim else '-'}`"
                   + (f", {frames:g} frames" if frames else "")
                   + ("" if aloc else "  **not local**" if anim else ""))
        bits = [f"cancel frame {cancel}" if cancel else None,
                f"intangible {x0}-{x1}" if x0 or x1 else None, f"flags 0x{flags:x}" if flags else None]
        if any(bits):
            out.append("- motion list: " + ", ".join(b for b in bits if b))
        sts = self.q("select distinct status from status_motions where fighter=? and motion=?", f, kind)
        if sts:
            out.append("- played by status: " + ", ".join(f"`{s[0]}`" for s in sts))
        allscripts = [game] + json.loads(scripts)
        found = [s for s in allscripts if self.q("select 1 from acmd where fighter=? and script=? limit 1", f, s)]
        out.append("- ACMD: " + (", ".join(f"`{s}`" for s in found) if found else "none dumped"))
        hb = self.q("select frame, until, id, part, bone, damage, angle, kbg, fkb, bkb, size, attr, cond, script "
                    "from hitboxes where fighter=? and script=? order by frame, id", f, game)
        if hb:
            out.append("\n| frames | id | dmg | angle | kbg | fkb | bkb | size | bone | attr |")
            out.append("|---|---|---|---|---|---|---|---|---|---|")
            for r in hb:
                w = f"{fmt(r[0])}–{fmt(r[1]) if r[1] is not None else '…'}" + (" (cond)" if r[12] else "")
                out.append(f"| {w} | {r[2]} | {r[5]} | {r[6]} | {r[7]} | {r[8]} | {r[9]} | {r[10]} | {r[4]} | "
                           f"{r[11].replace('collision_attr_', '')} |")
        for s in found:
            if s.startswith("effect_"):
                ef = self.q("select frame, call, effect, bone, color, cond from effects where fighter=? and script=? "
                            "order by frame", f, s)
                if ef:
                    out.append("- effects: " + "; ".join(
                        f"{fmt(e[0])}: {e[1]} `{e[2] or ''}`" + (f" @{e[3]}" if e[3] else "")
                        + (f" colour {e[4]}" if e[4] else "") + (" (cond)" if e[5] else "") for e in ef[:30])
                        + (" …" if len(ef) > 30 else ""))
            if s.startswith("sound_"):
                so = self.q("select frame, sound from sounds where fighter=? and script=? order by frame", f, s)
                if so:
                    out.append("- sounds: " + "; ".join(f"{fmt(x[0])}: `{x[1]}`" for x in so[:20]))
        ar = self.q("select frame, call, article from articles where fighter=? and script in (%s)"
                    % ",".join("?" * len(found)), f, *found) if found else []
        if ar:
            out.append("- articles: " + "; ".join(f"{fmt(x[0])}: {x[1]} {x[2] or ''}" for x in ar))
        return "\n".join(out) + "\n"

    MOTION_COLS = ("agent, kind, game_script, scripts, cancel_frame, xlu_start, xlu_end, animation, anim_local, "
                   "anim_frames, acmd_agent, flags")

    def sheet(self, f):
        fr = self.q("select * from fighters where fighter=?", f)
        if not fr:
            return f"# {f}\n\nnot indexed\n"
        _, agents, scripts, motions, statuses, params, assets, assets_local = fr[0]
        out = [f"# {f}\n",
               f"Generated by `tools/move-index.py build` from this PC's copies of the game's data. "
               f"{motions} c00 motions, {scripts} ACMD scripts across {agents} agents, {statuses} registered "
               f"status functions, {params} param values.\n"]
        miss = self.q("select category, count(*), sum(local_path is not null) from assets where fighter=? group by category", f)
        out.append("| assets | in the game | on this PC |\n|---|---|---|")
        out += [f"| {c} | {n} | {l} |" for c, n, l in miss]
        out.append("")
        rows = self.q(f"select {self.MOTION_COLS} from motions where fighter=? order by agent != 'body', agent, kind", f)
        for r in rows:
            if self.q("select 1 from acmd where fighter=? and script=? limit 1", f, r[2]):
                out.append(self.motion_block(f, r))
        return "\n".join(out)


def cmd_move(a):
    ix = Index()
    rows = ix.q(f"select {Index.MOTION_COLS} from motions where fighter=? and kind like ? order by kind",
                a.fighter, f"%{a.motion}%")
    if not rows:
        sys.exit(f"no motion matching {a.motion!r} for {a.fighter}")
    for r in rows:
        print(ix.motion_block(a.fighter, r))


def cmd_moves(a):
    ix = Index()
    for agent, kind, game, frames, aloc in ix.q("select agent, kind, game_script, anim_frames, anim_local from motions "
                                                 "where fighter=? order by agent, kind", a.fighter):
        has = bool(ix.q("select 1 from acmd where fighter=? and script=? limit 1", a.fighter, game))
        nh = ix.q("select count(*) from hitboxes where fighter=? and script=?", a.fighter, game)[0][0]
        print(f"{agent:<14} {kind:<36} {fmt(frames):>6}  {'acmd' if has else '    '}  "
              f"{str(nh) + ' hitboxes' if nh else ''}{'' if aloc else '  (animation not local)'}")


def cmd_missing(a):
    ix = Index()
    for cat, n, l in ix.q("select category, count(*), sum(local_path is not null) from assets where fighter=? "
                          "group by category order by category", a.fighter):
        print(f"{cat:<8} {l:>6} of {n:>6} local")
    if a.list:
        for (p,) in ix.q("select arc_path from assets where fighter=? and local_path is null and category like ? "
                         "order by arc_path", a.fighter, a.list):
            print("  ", p)


def cmd_sql(a):
    for r in Index().con.execute(a.query):
        print("\t".join(fmt(v) for v in r))


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("build").set_defaults(f=cmd_build)
    p = sub.add_parser("move"); p.add_argument("fighter"); p.add_argument("motion"); p.set_defaults(f=cmd_move)
    p = sub.add_parser("moves"); p.add_argument("fighter"); p.set_defaults(f=cmd_moves)
    p = sub.add_parser("missing"); p.add_argument("fighter")
    p.add_argument("--list", help="also list missing paths of this category (model, motion, effect, sound)")
    p.set_defaults(f=cmd_missing)
    p = sub.add_parser("sql"); p.add_argument("query"); p.set_defaults(f=cmd_sql)
    a = ap.parse_args()
    a.f(a)


if __name__ == "__main__":
    main()
