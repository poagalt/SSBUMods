#!/usr/bin/env python3
"""Give Pikachu a LEFT-hand copy of each sword: the sword he shows when facing LEFT.

WHAT THE COPIES ARE FOR NOW (2026-10-18). They are no longer shown together with the right-hand
mesh. Pikachu's turnaround mirrors his animation, so facing left his visible LEFT hand swings and a
mesh on `HaveR` is drawn at the idle hand -- the "sword in the wrong hand" of every facing-left
playtest. src/sword shows the `HaveR` mesh facing right and only this `HaveL` copy facing left,
which is one sword in the swinging hand both ways (see `mesh_l` in src/sword/mod.rs). The name is
historical; the copies and this tool are unchanged.

THE ORIGINAL RATIONALE, kept for the record:

THE PROBLEM THIS SOLVES. A weapon single-bound to `HaveR` is geometrically correct in both
facings -- Part 9 proves that, and facing is a rigid Y rotation -- but `HaveR` sits 3.958 units off
Pikachu's centreline in the lateral axis, so facing swings the hand through about eight units of
camera depth on a body roughly two deep. The sword is in front of his silhouette one way and behind
it the other. That is geometry, not a bug, and no attachment mechanism fixes it.

Dual-wielding does: with a sword in each hand there is one in front whichever way he faces.

WHY NOT GEORGE'S EXACT TECHNIQUE. The reference mod bolts its sword on permanently -- it is always
there, on `HaveL`, `HaveR`, `HandR`, `ShoulderL` and `ShoulderR` -- because George carries ONE
weapon. This port swaps between five models and shows each only for its own moves, so the meshes
have to stay toggleable and a permanent graft is not available. What transfers is the idea, not the
implementation: put geometry on both hands and stop fighting handedness.

WHY THIS EDITS THE SHIPPED MODEL INSTEAD OF EXTENDING tools/merge-weapons.sh.

The merged model on disk and that tool DISAGREE. Its table binds chrom to `Sword`, cloud to
`fusionsword1` and mythra to `Sword1`, while the shipped `model.numshb` has all three on `HaveR`,
and `build.sh` does not run the merge -- the model is a committed artifact. So the tool would
regenerate a model whose binds differ from the one that has actually been playtested, and that
tool's own header records the playtest retiring the `HaveR` bake TWICE. Re-running it is not a
no-op and could not be verified without another boot.

So this appends, and touches nothing that exists. Every original mesh object, material, bounding
sphere and modl entry is left byte-identical; the only change is N new mesh objects with new names.
If it turns out wrong, deleting the `_l` rows restores exactly what shipped.

THE LEFT MESH IS THE SAME VERTICES. No mirroring, no bake. A rigid mesh single-bound to bone B
renders at `v . world(B)`, so binding the unmodified vertices to `HaveL` renders the sword in the
left hand at the mirror of where the right-hand copy sits -- because `HaveL`'s bind frame IS
`HaveR`'s mirrored (measured: x +3.959 against -3.958, y and z within 0.013). A blade is near
enough symmetric that the handedness of the model does not read.

`HaveL` is also the one bone with direct evidence behind it: merge-weapons.sh records realising
Palutena's staff onto the vanilla `HaveL` as "the one change that made a broken weapon work in one
direction".

  python tools/dual-wield.py            add the left-hand meshes (idempotent)
  python tools/dual-wield.py --verify   check the shipped model against this recipe
  python tools/dual-wield.py --remove   strip them again
"""
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SSBH = os.path.join(ROOT, "vendor", "ssbh_lib", "ssbh_data_json.exe")
MODEL = os.path.join(ROOT, "pikachu-moveset", "mod", "fighter", "pikachu",
                     "model", "body", "c00")
WORK = os.path.join(ROOT, "work", "dualwield")

# The swords, and only the swords. `palutena_staff` is deliberately absent: a staff is a two-handed
# prop held in one hand and a second copy would read as an error rather than as a flourish.
SUFFIX = "_l"
HAND = "HaveL"
WEAPONS = ["chrom_sword", "cloud_sword", "mythra_sword", "corrin_sword"]


# `ssbh_data_json` DISPATCHES ON THE FILE EXTENSION, and does not know `.nusrcmdlb`. It is a MODL
# file with the same `HBSS`/`LDOM` magic as `.numdlb` -- merge-weapons.sh established that -- so it
# is round-tripped through a `.numdlb`-named copy. Without this the tool simply writes no JSON and
# the read fails one line later with a missing file, which points nowhere near the cause.
import shutil

ALIAS = {"nusrcmdlb": "numdlb"}


def run(src, out):
    ext = src.rsplit(".", 1)[-1]
    if ext in ALIAS:
        tmp = os.path.join(WORK, "in." + ALIAS[ext])
        shutil.copyfile(src, tmp)
        src = tmp
    subprocess.run([SSBH, src, out], check=True, capture_output=True)
    with open(out, encoding="utf-8") as f:
        return json.load(f)


def write(data, tmp, dst):
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(data, f)
    ext = dst.rsplit(".", 1)[-1]
    real = dst
    if ext in ALIAS:
        dst = os.path.join(WORK, "out." + ALIAS[ext])
    subprocess.run([SSBH, tmp, dst], check=True, capture_output=True)
    if dst != real:
        shutil.copyfile(dst, real)


def load_all():
    os.makedirs(WORK, exist_ok=True)
    out = {}
    for ext in ("numshb", "numdlb", "nusrcmdlb", "numshexb"):
        out[ext] = run(os.path.join(MODEL, "model." + ext),
                       os.path.join(WORK, "cur.%s.json" % ext))
    return out


def save_all(d):
    for ext in ("numshb", "numdlb", "nusrcmdlb", "numshexb"):
        write(d[ext], os.path.join(WORK, "out.%s.json" % ext),
              os.path.join(MODEL, "model." + ext))


def present(d):
    return sorted({o["name"] for o in d["numshb"]["objects"] if o["name"].endswith(SUFFIX)})


def add():
    d = load_all()
    have = present(d)
    added = []
    for w in WEAPONS:
        ln = w + SUFFIX
        if ln in have:
            print("  -   %-16s already present" % ln)
            continue
        subs = [o for o in d["numshb"]["objects"] if o["name"] == w]
        if not subs:
            sys.exit("  !! %s is not in the shipped model" % w)
        # Bind to the LEFT hand. Everything else about the object is copied untouched, including
        # every vertex attribute, so the left sword is the right sword's data and cannot drift
        # from it.
        for o in subs:
            c = json.loads(json.dumps(o))
            c["name"] = ln
            c["parent_bone_name"] = HAND
            d["numshb"]["objects"].append(c)
        # modl entries: one per subindex, naming the same material as the right-hand copy.
        for lst in (d["numdlb"]["entries"], d["nusrcmdlb"]["entries"]):
            for e in [x for x in list(lst) if x["mesh_object_name"] == w]:
                c = dict(e)
                c["mesh_object_name"] = ln
                lst.append(c)
        # numshexb: the bounding sphere is in the BIND BONE's frame, and `HaveL` is `HaveR`
        # mirrored, so the same sphere bounds the same geometry. Copied rather than recomputed for
        # that reason -- and because a wrong sphere culls the mesh instead of misplacing it, which
        # is a bug that only shows at certain camera angles.
        for g in [x for x in list(d["numshexb"]["mesh_object_groups"])
                  if x["mesh_object_full_name"] == w]:
            c = json.loads(json.dumps(g))
            c["mesh_object_full_name"] = ln
            c["mesh_object_name"] = ln
            d["numshexb"]["mesh_object_groups"].append(c)
        added.append((ln, len(subs)))
    if not added:
        print("  nothing to do")
        return 0
    save_all(d)
    for ln, n in added:
        print("  added %-18s %d subindex/es on %s" % (ln, n, HAND))
    return 0


def remove():
    d = load_all()
    if not present(d):
        print("  nothing to remove")
        return 0
    d["numshb"]["objects"] = [o for o in d["numshb"]["objects"]
                              if not o["name"].endswith(SUFFIX)]
    for k in ("numdlb", "nusrcmdlb"):
        d[k]["entries"] = [e for e in d[k]["entries"]
                           if not e["mesh_object_name"].endswith(SUFFIX)]
    d["numshexb"]["mesh_object_groups"] = [
        g for g in d["numshexb"]["mesh_object_groups"]
        if not g["mesh_object_full_name"].endswith(SUFFIX)]
    save_all(d)
    print("  removed every %s mesh" % SUFFIX)
    return 0


def verify():
    d = load_all()
    by = {}
    for o in d["numshb"]["objects"]:
        by.setdefault(o["name"], []).append(o)
    bad = []
    for w in WEAPONS:
        ln = w + SUFFIX
        if ln not in by:
            bad.append("%s is missing" % ln)
            continue
        if len(by[ln]) != len(by.get(w, [])):
            bad.append("%s has %d subindex/es, %s has %d"
                       % (ln, len(by[ln]), w, len(by.get(w, []))))
            continue
        for l, r in zip(by[ln], by[w]):
            if l["parent_bone_name"] != HAND:
                bad.append("%s binds to %s, expected %s" % (ln, l["parent_bone_name"], HAND))
            # The whole point: the left mesh must be the right mesh's data, unmodified.
            if l["positions"] != r["positions"]:
                bad.append("%s vertices differ from %s" % (ln, w))
        for k in ("numdlb", "nusrcmdlb"):
            n = len([e for e in d[k]["entries"] if e["mesh_object_name"] == ln])
            if n != len(by[ln]):
                bad.append("%s has %d %s entries, expected %d" % (ln, n, k, len(by[ln])))
        n = len([g for g in d["numshexb"]["mesh_object_groups"]
                 if g["mesh_object_full_name"] == ln])
        if n != 1:
            bad.append("%s has %d numshexb group(s), expected 1" % (ln, n))
    for b in bad:
        print("  FAIL " + b, file=sys.stderr)
    if bad:
        return 1
    tot = sum(len(by[w + SUFFIX]) for w in WEAPONS)
    print("  ok  %d left-hand mesh object(s) across %d sword(s), all on %s, all vertex-identical "
          "to their right-hand copy" % (tot, len(WEAPONS), HAND))
    return 0


if __name__ == "__main__":
    a = sys.argv[1] if len(sys.argv) > 1 else ""
    sys.exit(verify() if a == "--verify" else remove() if a == "--remove" else add())
