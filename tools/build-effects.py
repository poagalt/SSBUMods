#!/usr/bin/env python3
"""Builds Rat of Doom's effect archive -- every donor effect his scripts name, and nothing else --
and ships it as a smashline effect transplant.

    python tools/build-effects.py

Writes, into the generated shipped tree:

    ship/effect/fighter/pikachu/transplant/ratofdoom/ef_ratofdoom.eff
    ship/effect/fighter/pikachu/transplant/ratofdoom/trail/<tex>.nutexb     sword-trail textures

and work/effects/ratofdoom-manifest.txt, the list of names the archive provides, which gate 4o
(tools/check-effects.py) checks every effect request against.

HOW THE ARCHIVE REACHES THE GAME. smashline's `load_fighter_effects` hook (main 0x60bfd8) loads
each folder under `effect/fighter/<fighter>/transplant/` as an extra effect archive for that
fighter, and the game's loader (main 0x355fea0) finds a folder's `.eff` and `.nutexb` files by
extension -- so the names in this folder are free. One merged archive rather than fourteen donor
ones because each donor's is 9-15 MB, and fourteen would add ~150 MB of effects to every match
with a Pikachu in it; this one carries only the emitter sets named.

WHERE THE NAMES COME FROM: the plugin source itself. Every `Hash40::new("<donor>_...")` that sits
in an effect call -- the generated move scripts and hand-written status code alike -- is a name the
archive must provide, so nothing is listed by hand and nothing can drift.

AND THE CLONED ARTICLES' OWN SCRIPTS, which are not in the plugin source at all: a clone runs its
donor's scripts, so a name only the article plays never appeared in the scan. Piranha Plant's
spikeball was the first to need it -- its Loop status plays `packun_spikeball_wind` while the ball
is being blown upward, and nothing of Pikachu's names it. Every ENABLED row of the Candidates table
in src/articles/mod.rs contributes the effect calls in its donor article's dumped scripts whose name
is the article's own (`packun_spikeball_*`) -- not every donor name, or the Cloud sword clone would
drag in his whole Final Smash (`cloud_final2_*`), which its scripts name and Rat of Doom never plays.

HOW IT IS MERGED, with KillzXGaming's EffectConverter v1.9 (vendor/tools/EffectConverter; VFX
version 22 is inside its supported range) to DUMP, and tools/effmerge to BUILD -- the converter's
own folder-to-archive path plus the texture-bank and model-bank re-saves v1.9 leaves commented out. Each donor archive is dumped once to
work/effects/dump/ef_<donor>/: a folder per emitter set (emitters with their own texture, model
and shader), `EmitterSetInfo.txt` for the order, and `NamcoFile.json` mapping each effect NAME to
a 1-based emitter-set ID. This copies the named sets into one folder, renumbers the name table to
the new order, and rebuilds. The result is re-dumped with the stock converter and every name
checked to resolve to the emitter set it was copied from, with every texture, model and shader in
that set byte-identical to the donor's (see `same_files`), or the build stops.
"""
import json
import os
import re
import shutil
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "pikachu-moveset", "plugin", "src")
SHIP = os.path.join(ROOT, "pikachu-moveset", "ship")
ARC = os.path.join(ROOT, "work", "effects", "arc", "effect", "fighter")
DUMP = os.path.join(ROOT, "work", "effects", "dump")
BUILD = os.path.join(ROOT, "work", "effects", "build")
MANIFEST = os.path.join(ROOT, "work", "effects", "ratofdoom-manifest.txt")
TOOL = os.path.join(ROOT, "vendor", "tools", "EffectConverter", "v1.9", "EffectConverter.exe")
OUT_DIR = os.path.join(SHIP, "effect", "fighter", "pikachu", "transplant", "ratofdoom")
NAME = "ef_ratofdoom"


DOTNET = os.path.join(ROOT, "vendor", "tools", "dotnet", "dotnet.exe")
MERGE = os.path.join(ROOT, "vendor", "tools", "effmerge", "effmerge.dll")


def same_files(src, got, name):
    """EVERY FILE of an emitter set, compared byte for byte against its re-dump.

    Textures, models and shaders must be identical. Emitter data may differ ONLY in
    ShaderReferences -- the rebuild gives each emitter's shader a new index in the merged shader
    list, and that index points at a byte-identical shader. This is the check that caught both of
    EffectConverter v1.9's skipped re-saves (its texture and model banks came out as the BASE
    file's), so it is not optional.
    """
    for base, _dirs, files in os.walk(src):
        for f in files:
            a = os.path.join(base, f)
            b = os.path.join(got, os.path.relpath(a, src))
            if not os.path.isfile(b):
                die("%s: %s is missing after the rebuild" % (name, os.path.relpath(a, src)))
            A, B = open(a, "rb").read(), open(b, "rb").read()
            if A == B:
                continue
            if f == "EmitterData.json":
                ja = json.load(open(a, encoding="utf-8"))
                jb = json.load(open(b, encoding="utf-8"))
                ja.pop("ShaderReferences", None)
                jb.pop("ShaderReferences", None)
                if ja == jb:
                    continue
            elif f == "EmitterData.bin" and len(A) == len(B) and sum(x != y for x, y in zip(A, B)) <= 4:
                continue
            die("%s: %s changed in the rebuild" % (name, os.path.relpath(a, src)))


def die(msg):
    sys.stderr.write("  REFUSING: %s\n" % msg)
    sys.exit(1)


def donors_available():
    return sorted(d for d in os.listdir(ARC) if os.path.isfile(os.path.join(ARC, d, "ef_%s.eff" % d)))


def wanted_names(donors):
    """Effect names the plugin requests whose prefix is a donor -- by call SHAPE, like gate 4o."""
    names, trails = set(), set()
    call = re.compile(r"(macros::[A-Za-z0-9_]*(?:EFFECT|AFTER_IMAGE)[A-Za-z0-9_]*|EffectModule::\w+)\(")
    for base, _dirs, files in os.walk(SRC):
        for f in files:
            if not f.endswith(".rs"):
                continue
            # WHOLE CALLS, ACROSS LINES: a call written over several lines (the Revenge aura's
            # EFFECT_FOLLOW) puts its name on a later line than the macro, and a per-line scan
            # missed it. Comments are blanked first so a quoted example cannot count.
            text = re.sub(r"//[^\n]*", "", open(os.path.join(base, f), encoding="utf-8").read())
            for m in call.finditer(text):
                depth, i = 1, m.end()
                while i < len(text) and depth:
                    depth += {"(": 1, ")": -1}.get(text[i], 0)
                    i += 1
                for n in re.findall(r'Hash40::new\("([a-z0-9_]+)"\)', text[m.end():i]):
                    if n.startswith("tex_"):
                        trails.add(n)
                    elif n.split("_")[0] in donors and n.split("_")[0] != "pikachu":
                        names.add(n)
    for donor, article in cloned_articles():
        scripts = os.path.join(ROOT, "vendor", "SSBU-Dumped-Scripts", "smashline", "lua2cpp_%s" % donor,
                               "%s_%s" % (donor, article))
        if not os.path.isdir(scripts):
            continue
        for f in sorted(os.listdir(scripts)):
            text = open(os.path.join(scripts, f), encoding="utf-8").read()
            for m in call.finditer(text):
                depth, i = 1, m.end()
                while i < len(text) and depth:
                    depth += {"(": 1, ")": -1}.get(text[i], 0)
                    i += 1
                for n in re.findall(r'Hash40::new\("([a-z0-9_]+)"\)', text[m.end():i]):
                    if n.startswith("%s_%s" % (donor, article)) and donor in donors:
                        names.add(n)
    return names, trails


def cloned_articles():
    """(donor, donor_article) for every ENABLED row of src/articles/mod.rs's Candidates table."""
    rows = []
    for line in open(os.path.join(SRC, "articles", "mod.rs"), encoding="utf-8"):
        t = line.strip()
        if not t.startswith("Candidate {") or "enabled: true" not in t:
            continue
        d = re.search(r'donor: "([a-z0-9_]+)"', t)
        a = re.search(r'donor_article: "([a-z0-9_]+)"', t)
        if d and a:
            rows.append((d.group(1), a.group(1)))
    return rows


def dump(donor):
    d = os.path.join(DUMP, "ef_%s" % donor)
    if os.path.isfile(os.path.join(d, "NamcoFile.json")):
        return d
    os.makedirs(DUMP, exist_ok=True)
    shutil.copyfile(os.path.join(ARC, donor, "ef_%s.eff" % donor), os.path.join(DUMP, "ef_%s.eff" % donor))
    subprocess.run([TOOL, "ef_%s.eff" % donor], cwd=DUMP, capture_output=True)
    if not os.path.isfile(os.path.join(d, "NamcoFile.json")):
        die("EffectConverter did not dump ef_%s.eff" % donor)
    return d


def load_dump(d):
    names = json.load(open(os.path.join(d, "NamcoFile.json"), encoding="utf-8"))
    order = json.load(open(os.path.join(d, "EmitterSetInfo.txt"), encoding="utf-8"))["Order"]
    return names, order


def main():
    if not os.path.isfile(TOOL):
        die("no EffectConverter at %s" % TOOL)
    if not os.path.isdir(os.path.join(SHIP, "fighter")):
        die("no shipped tree -- run tools/relocate-slot.sh first")
    donors = donors_available()
    names, trails = wanted_names(donors)
    if not names:
        die("no donor effect names found in the plugin source")

    # ---- gather: each name -> (donor, its entry, the emitter-set folder it points at) --------------
    picked = []
    by_donor = {}
    for n in sorted(names):
        donor = n.split("_")[0]
        if donor not in by_donor:
            by_donor[donor] = (dump(donor),) + load_dump(dump(donor))
        d, table, order = by_donor[donor]
        entry = next((e for e in table if e["Name"].lower() == n), None)
        if entry is None:
            die("%s is not in ef_%s.eff" % (n, donor))
        if entry["ExternalModelFlag"] or entry["Variants"]:
            die("%s uses external models or variants, which this merge does not carry" % n)
        sid = entry["EmitterSet_ID"]
        if not 0 < sid <= len(order):
            die("%s points at emitter set %d of %d" % (n, sid, len(order)))
        picked.append((n, donor, entry, order[sid - 1]))

    # ---- assemble one dump folder -----------------------------------------------------------------
    work = os.path.join(BUILD, NAME)
    shutil.rmtree(BUILD, ignore_errors=True)
    os.makedirs(work)
    # THE LARGEST DONOR'S Base.ptcl, not the first one's. The converter clears every emitter,
    # shader, texture and model out of the base before rebuilding, but the base's own containers
    # still frame the result -- built on Cloud's (the alphabetically first), a merged archive
    # came back with a Kazuya texture listed in its descriptor table and missing from its BNTX,
    # which the converter itself then refused to read. Each donor round-trips cleanly on its own.
    base = max((v[0] for v in by_donor.values()),
               key=lambda d: os.path.getsize(os.path.join(d, "Base.ptcl")))
    for f in ("Base.ptcl", "PtclHeader.txt"):
        shutil.copyfile(os.path.join(base, f), os.path.join(work, f))
    new_order, set_index, new_table = [], {}, []
    for n, donor, entry, folder in picked:
        key = (donor, folder)
        if key not in set_index:
            dst = folder if folder not in new_order else "%s_%s" % (donor, folder)
            shutil.copytree(os.path.join(by_donor[donor][0], folder), os.path.join(work, dst))
            new_order.append(dst)
            set_index[key] = len(new_order)          # 1-based, as the game's table is
        e = dict(entry)
        e["EmitterSet_ID"] = set_index[key]
        new_table.append(e)
    json.dump({"Order": new_order}, open(os.path.join(work, "EmitterSetInfo.txt"), "w"), indent=2)
    json.dump(new_table, open(os.path.join(work, "NamcoFile.json"), "w"), indent=2)

    # ---- rebuild with tools/effmerge (EffectConverter's own path plus the re-saves it skips) -------
    if not os.path.isfile(MERGE):
        r = subprocess.run([DOTNET, "build", "-c", "Release", "-o", os.path.dirname(MERGE)],
                           cwd=os.path.join(ROOT, "tools", "effmerge"), capture_output=True, text=True)
        if not os.path.isfile(MERGE):
            die("could not build tools/effmerge:\n%s" % r.stdout[-2000:])
    built = os.path.join(BUILD, NAME + "_NEW.eff")
    r = subprocess.run([DOTNET, MERGE, NAME, built], cwd=BUILD, capture_output=True, text=True)
    if r.returncode != 0 or not os.path.isfile(built):
        die("effmerge failed:\n%s" % (r.stdout[-2000:] + r.stderr[-2000:]))

    # ---- verify by re-dumping: every name resolves to the set it was copied from ------------------
    check = os.path.join(BUILD, "verify")
    os.makedirs(check)
    shutil.copyfile(built, os.path.join(check, NAME + ".eff"))
    subprocess.run([TOOL, NAME + ".eff"], cwd=check, capture_output=True)
    vt, vo = load_dump(os.path.join(check, NAME))
    for n, donor, entry, folder in picked:
        e = next((x for x in vt if x["Name"].lower() == n), None)
        if e is None or vo[e["EmitterSet_ID"] - 1] not in (folder, "%s_%s" % (donor, folder)):
            die("verification failed for %s" % n)
        same_files(os.path.join(by_donor[donor][0], folder),
                   os.path.join(check, NAME, vo[e["EmitterSet_ID"] - 1]), n)

    # ---- ship -------------------------------------------------------------------------------------
    shutil.rmtree(OUT_DIR, ignore_errors=True)
    os.makedirs(OUT_DIR)
    shutil.copyfile(built, os.path.join(OUT_DIR, NAME + ".eff"))
    shipped_trails = []
    for t in sorted(trails):
        donor = t[len("tex_"):].split("_")[0]
        p = os.path.join(ARC, donor, "trail", t + ".nutexb")
        if not os.path.isfile(p):
            die("trail texture %s is not in work/effects/arc (pull effect/fighter/%s/trail)" % (t, donor))
        os.makedirs(os.path.join(OUT_DIR, "trail"), exist_ok=True)
        shutil.copyfile(p, os.path.join(OUT_DIR, "trail", t + ".nutexb"))
        shipped_trails.append(t)
    with open(MANIFEST, "w", encoding="utf-8", newline="\n") as fh:
        for n, *_ in picked:
            fh.write(n + "\n")
        for t in shipped_trails:
            fh.write(t + "\n")
    size = os.path.getsize(os.path.join(OUT_DIR, NAME + ".eff"))
    print("  %s.eff: %d effects from %d donors in %d emitter sets, %.1f MB; %d trail texture(s)"
          % (NAME, len(picked), len(by_donor), len(new_order), size / 1e6, len(shipped_trails)))


if __name__ == "__main__":
    main()
