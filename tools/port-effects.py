#!/usr/bin/env python3
"""Ports a source fighter's `effect_` script to Rat of Doom, as faithfully as the engine allows.

    python tools/port-effects.py <source dump .txt> effect_<name> [--report FILE]

Prints the function BODY (the caller writes the signature and closing brace). Replaces
tools/extract-effects.awk, which kept only `sys_*` spawns under ADR 0002.

WHY THE DONOR'S OWN EFFECTS ARE ALLOWED NOW. ADR 0002 forbade them because a fighter's effect
archive is only resident when that fighter is in the match, and the brief forbade depending on the
CSK Collection. Rat of Doom is a CSK slot now, and smashline ships the missing piece: its
`load_fighter_effects` hook (main 0x60bfd8) loads every folder under
`effect/fighter/<fighter>/transplant/` as an extra effect archive for that fighter. The game's own
loader (main 0x355fea0, decompiled) picks the `.eff` in a folder by EXTENSION and its `.nutexb`
trails the same way, so tools/build-effects.py merges exactly the emitter sets these scripts name
into one archive and ships it there. The names below are the donors' own, verbatim -- the
Smash Ultimate Modding Workshop's episode 46 does the same thing (Chrom calling
`elight_attack100_finish` through `effect/fighter/chrom/transplant/elight/ef_elight.eff`).

WHAT IS KEPT VERBATIM: frame/wait timing, every effect macro (spawns, LAST_EFFECT_SET_* and
LAST_PARTICLE_SET_COLOR modifiers, AFTER_IMAGE trails, EFFECT_OFF_KIND kills, FLASH/COL_* tints),
`EffectModule::enable_sync_init_pos_last`, `for` loops, and the facing branch
(`get_value_float(..., SO_VAR_FLOAT_LR)`), all of which compile as the dump wrote them.

WHAT IS TRANSLATED, each for a measured reason:
  * COSTUME-COLOUR BRANCHES (`WorkModule::get_int(.., FIGHTER_INSTANCE_WORK_ID_INT_COLOR) == N`) are
    resolved at generation time to the c00 branch. Rat of Doom's colour index is 120, which matches
    none of them, so copying them would drop every per-costume tint; c00 is the donor's own default.
  * JOINTS PIKACHU DOES NOT HAVE AT RUNTIME (checked against vanilla model.nusktb, not the shipped
    skeleton, whose grafted bones do not exist in game):
      - `feeler`/`feelerca` (Incineroar's head whiskers) -> `head`.
      - `sword1` (Pyra/Mythra) -> `haver`. Their skeletons put Sword1 on HaveR with NO translation
        and a 90-degree turn about Z (rows (0,1,0) (-1,0,0) (0,0,1)), and their blade is merged
        onto Pikachu's HaveR with that transform baked in. So an offset (x,y,z) becomes (-y,x,z)
        and, with the effect system's X-then-Y-then-Z rotation order, a rotation (rx,ry,rz)
        becomes (rx,ry,rz+90). Exact, not approximate.
  * `EFFECT_FOLLOW_WORK` / `EFFECT_OFF_KIND_WORK` read the effect from a work slot only the DONOR'S
    code fills (Cloud's FIGHTER_CLOUD_INSTANCE_WORK_ID_INT_EFFECT_KIND_SWORD_FLARE). Nothing fills
    it on Pikachu, so they become the named effect Cloud's normal state puts there.

ANYTHING ELSE FAILS. A statement shape this does not recognise is reported as UNKNOWN on stderr
(and in --report), and tools/gen-moves.sh stops -- a silently dropped effect is the failure mode
this pipeline exists to prevent.
"""
import re
import sys

JOINT_MAP = {"feeler": "head", "feelerca": "head"}
SWORD1 = "sword1"
WORK_EFFECT = {
    "*FIGHTER_CLOUD_INSTANCE_WORK_ID_INT_EFFECT_KIND_SWORD_FLARE": "cloud_sword_flare",
}
# Macros whose argument list is (agent, name, joint, x, y, z, rx, ry, rz, ...): the joint and its
# offset are rewritten together when the joint is sword1.
POSED = re.compile(r"^(EFFECT|EFFECT_FOLLOW|EFFECT_FOLLOW_NO_STOP|EFFECT_FOLLOW_FLIP|EFFECT_FLIP|"
                   r"EFFECT_ALPHA|EFFECT_FOLLOW_ALPHA|EFFECT_FLIP_ALPHA|EFFECT_FOLLOW_FLIP_ALPHA|"
                   r"LANDING_EFFECT|LANDING_EFFECT_FLIP|FOOT_EFFECT)$")
KEEP_MACRO = re.compile(r"(EFFECT|AFTER_IMAGE|FLASH|COL_NORMAL|BURN_COLOR|LAST_PARTICLE)")
COLOR_IF = re.compile(r"^if WorkModule::get_int\(agent\.module_accessor, \*FIGHTER_INSTANCE_WORK_ID_INT_COLOR\) "
                      r"(==|!=) (\d+) \{$")


def split_args(s):
    """Top-level comma split of a call's argument text."""
    out, depth, cur = [], 0, ""
    for ch in s:
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
        if ch == "," and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += ch
    if cur.strip():
        out.append(cur.strip())
    return out


def num(s):
    return float(s.replace(" ", ""))


def fmt(v):
    return ("%g" % v) if v != int(v) else str(int(v))


def remap_joint_arg(a):
    m = re.match(r'^Hash40::new\("([a-z0-9_]+)"\)$', a)
    if m and m.group(1) in JOINT_MAP:
        return 'Hash40::new("%s")' % JOINT_MAP[m.group(1)]
    return a


def sword_pos(args, i):
    """args[i..i+3] are x,y,z on sword1 -> haver."""
    x, y, z = num(args[i]), num(args[i + 1]), num(args[i + 2])
    args[i], args[i + 1], args[i + 2] = fmt(-y), fmt(x), fmt(z)


def rewrite_call(name, argtext):
    args = split_args(argtext)
    if name in ("EFFECT_FOLLOW_WORK", "EFFECT_OFF_KIND_WORK"):
        if args[1] not in WORK_EFFECT:
            return None, "UNKNOWN work-slot effect %s" % args[1]
        args[1] = 'Hash40::new("%s")' % WORK_EFFECT[args[1]]
        name = name.replace("_WORK", "")
    if POSED.match(name) and len(args) >= 9:
        if args[2] == 'Hash40::new("%s")' % SWORD1:
            args[2] = 'Hash40::new("haver")'
            sword_pos(args, 3)
            args[8] = fmt(num(args[8]) + 90)
    elif name.startswith("AFTER_IMAGE4_ON") and len(args) >= 22:
        # (agent, tex1, tex2, n, j1, x,y,z, j2, x,y,z, bool, name, j3, x,y,z, rx,ry,rz, ...)
        for j, p in ((4, 5), (8, 9), (14, 15)):
            if args[j] == 'Hash40::new("%s")' % SWORD1:
                args[j] = 'Hash40::new("haver")'
                sword_pos(args, p)
                if j == 14:
                    args[20] = fmt(num(args[20]) + 90)
    args = [remap_joint_arg(a) for a in args]
    return "macros::%s(%s);" % (name, ", ".join(args)), None


def port(lines, want):
    # The function ends where its braces balance, NOT at the first column-0 `}`: the dump prints
    # some inner closers unindented (Ness's back-throw `for` loop), and stopping there cut six
    # scripts short.
    body, infn, depth = [], False, 0
    for ln in lines:
        if not infn:
            if re.match(r'unsafe extern "C" fn %s\(' % re.escape(want), ln):
                infn, depth = True, ln.count("{") - ln.count("}")
            continue
        depth += ln.count("{") - ln.count("}")
        if depth <= 0:
            break
        body.append(ln.rstrip("\n"))
    out, problems = [], []
    # Block stack: each entry is (emit_close, keep_children).
    stack = []

    def live():
        return all(k for _, k in stack)

    last_closed = None
    for raw in body:
        s = raw.strip()
        if not s:
            continue
        # `}` and `else {` on separate lines, the way the dump prints them: the block that just
        # closed is re-opened as its else-arm. Rust accepts the newline, so an emitted branch
        # prints `else {` on its own line too.
        if s == "else {":
            if last_closed is None:
                problems.append("UNKNOWN statement: else { with no if before it")
                stack.append((False, False))
                continue
            emit, keep = last_closed
            last_closed = None
            if emit:
                if live():
                    out.append("    " * (len([1 for e, _ in stack if e]) + 1) + "else {")
                stack.append((True, True))
            else:
                stack.append((False, not keep))
            continue
        if s != "}":
            last_closed = None
        depth = "    " * (len([1 for e, _ in stack if e]) + 1)
        m = COLOR_IF.match(s)
        if m:
            op, n = m.group(1), int(m.group(2))
            stack.append((False, (n == 0) if op == "==" else (n != 0)))
            continue
        if s == "} else {":
            emit, keep = stack.pop()
            if emit:
                if live():
                    out.append("    " * (len([1 for e, _ in stack if e]) + 1) + "} else {")
                stack.append((True, keep))
            else:
                stack.append((False, not keep))
            continue
        if s.startswith("if ") and s.endswith("{"):
            if s == "if macros::is_excute(agent) {" or s.startswith("if get_value_float(agent.lua_state_agent, *SO_VAR_FLOAT_LR)"):
                if live():
                    out.append(depth + s)
                stack.append((True, True))
                continue
            problems.append("UNKNOWN condition: " + s)
            stack.append((False, False))
            continue
        if s.startswith("for _ in ") and s.endswith("{"):
            if live():
                out.append(depth + s)
            stack.append((True, True))
            continue
        if s == "}":
            if not stack:
                problems.append("UNBALANCED brace")
                continue
            emit, keep = stack.pop()
            if emit and live():
                out.append("    " * (len([1 for e, _ in stack if e]) + 1) + "}")
            last_closed = (emit, keep)
            continue
        if not live():
            continue
        if re.match(r"^(frame|wait)\(agent\.lua_state_agent, [0-9.]+\);$", s):
            out.append(depth + s)
            continue
        if s == "EffectModule::enable_sync_init_pos_last(agent.module_accessor);":
            out.append(depth + s)
            continue
        m = re.match(r"^macros::([A-Za-z0-9_]+)\((.*)\);$", s)
        if m and KEEP_MACRO.search(m.group(1)):
            line, err = rewrite_call(m.group(1), m.group(2))
            if err:
                problems.append(err)
                continue
            out.append(depth + line)
            continue
        problems.append("UNKNOWN statement: " + s)
    if stack:
        problems.append("UNBALANCED: %d block(s) left open" % len(stack))
    return out, problems


def main():
    args = sys.argv[1:]
    report = None
    if "--report" in args:
        i = args.index("--report")
        report = args[i + 1]
        del args[i:i + 2]
    src, want = args
    out, problems = port(open(src, encoding="utf-8").read().split("\n"), want)
    print("\n".join(out))
    if problems:
        for p in problems:
            sys.stderr.write("%s: %s\n" % (want, p))
        if report:
            with open(report, "a", encoding="utf-8") as fh:
                for p in problems:
                    fh.write("%s: %s\n" % (want, p))


if __name__ == "__main__":
    main()
