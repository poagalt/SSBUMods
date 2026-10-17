#!/usr/bin/env python3
"""Graft Pyra's own Aegis blade into Pikachu's model, for her d-air.

WHY THIS EXISTS. Pyra's d-air replaced K. Rool's, and it is a sword swing, so it was pointed at
`mythra_sword` -- the blade already merged and dual-wielded -- on the reasoning that Pyra and
Mythra are the same fighter pair. They are not the same SWORD: hers is the flame form, Mythra's
the light form, and the playtest said so. This ships the real one.

WHY IT IS A SURGICAL APPEND RATHER THAN A tools/merge-weapons.sh ROW.

`merge-weapons.sh` rebuilds Pikachu's model from vanilla plus its own table, and two things make
that unusable here. Its table and the shipped model DISAGREE -- it binds chrom to `Sword`, cloud to
`fusionsword1` and mythra to `Sword1` while the shipped model has all three on `HaveR` -- and
`build.sh` never runs it, so the model is a committed artifact that regenerating would silently
change. It would also destroy the `_l` meshes `tools/dual-wield.py` appends, which are working.

So this appends and touches nothing that exists, exactly as dual-wield.py does.

THE BAKE IS merge-weapons.sh's OWN ALGEBRA, and its preconditions were re-checked for Pyra rather
than assumed. A skinned mesh does not reduce the way a rigid one does: skinning would compute
`v . Wbind_pika(B)^-1 . W_pika(B)`, substituting PIKACHU's bind pose for the weapon model's hand
frame, which leaves the bone's deviation from his bind pose instead of its absolute pose -- the
mesh renders near the model origin and drifts as the arm moves. The fix is one matrix,
`inverse(world_esword(HaveR))`, which maps every vertex into the hand frame at bind pose.

One matrix is valid for vertices weighted to six different bones only because, measured here:

  - all 4611 of Pyra's skin weights are exactly 1.0 across `Sword1` and `Sword_flameA..E`, so
    there is no blending to lose; and
  - no shipped Pyra animation drives any of them. `c05attackairlw` carries 175 tracks and the ones
    matching "flame" are all material and expression tracks (`eflame_Mouth_*`, `skin_flame_001`),
    not the skeleton bones. The blade is rigid relative to `HaveR` for the whole move, and bind
    pose is the only pose those bones are ever in.

Both are the same facts merge-weapons.sh established for Mythra, re-derived for Pyra.

  python tools/graft-pyra-sword.py            add the meshes, materials and textures
  python tools/graft-pyra-sword.py --verify   check the shipped model against this recipe
  python tools/graft-pyra-sword.py --remove   strip it again
"""
import json
import os
import shutil
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SSBH = os.path.join(ROOT, "vendor", "ssbh_lib", "ssbh_data_json.exe")
MODEL = os.path.join(ROOT, "pikachu-moveset", "mod", "fighter", "pikachu", "model", "body", "c00")
# TWO SOURCE ROOTS, searched per FILE rather than per directory. `extracted/` holds only
# `model.nusktb` for this weapon -- the meshes, materials and spheres were pulled off the running
# console into `work/arc-weapons` with tools/arc-fetch.sh -- so neither root has the whole set and
# picking one directory finds half of it.
SRC_ROOTS = [os.path.join(ROOT, "work", "arc-weapons", "fighter", "eflame", "model", "esword", "c00"),
             os.path.join(ROOT, "extracted", "fighter", "eflame", "model", "esword", "c00")]


def src_file(name):
    for r in SRC_ROOTS:
        p = os.path.join(r, name)
        if os.path.exists(p):
            return p
    sys.exit("  !! %s is in neither %s -- pull it with tools/arc-fetch.sh"
             % (name, " nor ".join(SRC_ROOTS)))


SRC = SRC_ROOTS[0]
WORK = os.path.join(ROOT, "work", "pyrasword")

NAME = "pyra_sword"
LEFT = NAME + "_l"
SRC_MESH = "flamesword_VIS_O_OBJShape"
HAND, HAND_L = "HaveR", "HaveL"

# THIS TOOL IS THE SECOND THING THAT PUTS A MESH IN THE SHIPPED MODEL, and `tools/check-weapons.sh`
# was written when `tools/merge-weapons.sh` was the only one. Its first gate compares the merge
# table against `src/sword/mod.rs` and reported `pyra_sword` as "shown but not in the model" the
# moment this graft landed -- a false alarm, because the mesh is demonstrably there.
#
# Rather than teach the checker a name it would then hold on its own, the row this weapon would
# have had in the merge table is declared HERE, in the tool that actually creates the mesh, in the
# merge table's exact column order:
#
#     name | source model dir | source mesh object | material | bone | parent | cx,cy,cz | radius
#
# `check-weapons.sh` greps for it, so every one of its six table-driven gates now covers this
# weapon too -- the bind bone, the skinned-influence check, the bounding sphere and all eleven
# textures included. Adding the row to merge-weapons.sh instead was the other option and is worse:
# that tool REGENERATES the whole model from scratch, it is not a build step, and its own table
# already disagrees with the shipped binds, so a row there would claim this graft came from a run
# nobody has made.
#
# The sphere is the source article's own, copied by this script rather than guessed; the checker
# does not read those two columns (it measures the shipped vertices instead) but the row is written
# whole so it can be pasted into the merge table if these tools are ever unified.
TABLE_ROW = ("pyra_sword|eflame/model/esword|flamesword_VIS_O_OBJShape|alp_flame_005"
             "|HaveR|HandR|-0.0024335594,3.660273,-0.038366903|7.730845")
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
    real, out = dst, dst
    if ext in ALIAS:
        out = os.path.join(WORK, "out." + ALIAS[ext])
    subprocess.run([SSBH, tmp, out], check=True, capture_output=True)
    if out != real:
        shutil.copyfile(out, real)


def mul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(4)) for j in range(4)] for i in range(4)]


def world_bind(bones, i):
    m = bones[i]["transform"]
    p = bones[i].get("parent_index")
    while p is not None:
        m = mul(m, bones[p]["transform"])
        p = bones[p].get("parent_index")
    return m


def invert_rigid(m):
    """Inverse of a rotation+translation matrix in this row-vector convention."""
    r = [[m[i][j] for j in range(3)] for i in range(3)]
    t = m[3][:3]
    ri = [[r[j][i] for j in range(3)] for i in range(3)]          # transpose = inverse rotation
    ti = [-sum(t[k] * ri[k][i] for k in range(3)) for i in range(3)]
    return [ri[0] + [0.0], ri[1] + [0.0], ri[2] + [0.0], ti + [1.0]]


def xform_point(v, m):
    return [sum(v[k] * m[k][i] for k in range(3)) + m[3][i] for i in range(3)]


def xform_dir(v, m):
    return [sum(v[k] * m[k][i] for k in range(3)) for i in range(3)]


def load():
    os.makedirs(WORK, exist_ok=True)
    return {e: run(os.path.join(MODEL, "model." + e), os.path.join(WORK, "cur.%s.json" % e))
            for e in ("numshb", "numdlb", "nusrcmdlb", "numshexb", "numatb")}


def save(d):
    for e in ("numshb", "numdlb", "nusrcmdlb", "numshexb", "numatb"):
        write(d[e], os.path.join(WORK, "out.%s.json" % e), os.path.join(MODEL, "model." + e))


def bake_matrix():
    sk = run(src_file("model.nusktb"), os.path.join(WORK, "src.nusktb.json"))
    by = {b["name"]: i for i, b in enumerate(sk["bones"])}
    if HAND not in by:
        sys.exit("  !! eflame/model/esword has no %s bone to bake against" % HAND)
    return invert_rigid(world_bind(sk["bones"], by[HAND]))


def add():
    d = load()
    if any(o["name"] == NAME for o in d["numshb"]["objects"]):
        print("  -   %s already present" % NAME)
        return 0
    m = bake_matrix()
    smesh = run(src_file("model.numshb"), os.path.join(WORK, "src.numshb.json"))
    sdlb = run(src_file("model.numdlb"), os.path.join(WORK, "src.numdlb.json"))
    satb = run(src_file("model.numatb"), os.path.join(WORK, "src.numatb.json"))
    sshex = run(src_file("model.numshexb"), os.path.join(WORK, "src.numshexb.json"))

    subs = [o for o in smesh["objects"] if o["name"] == SRC_MESH]
    if not subs:
        sys.exit("  !! %s is not in %s" % (SRC_MESH, SRC))

    # Material labels, straight from the donor's own modl so the pairing cannot drift.
    mat_of = {e["mesh_object_subindex"]: e["material_label"]
              for e in sdlb["entries"] if e["mesh_object_name"] == SRC_MESH}
    have_mats = {e["material_label"] for e in d["numatb"]["entries"]}
    for e in satb["entries"]:
        if e["material_label"] in set(mat_of.values()) and e["material_label"] not in have_mats:
            d["numatb"]["entries"].append(json.loads(json.dumps(e)))

    nvert = 0
    for o in subs:
        for hand, nm in ((HAND, NAME), (HAND_L, LEFT)):
            c = json.loads(json.dumps(o))
            c["name"] = nm
            c["parent_bone_name"] = hand
            # SINGLE-BOUND, so the influences go. Keeping them would leave the mesh skinned to
            # bones Pikachu does not have, which is both wrong and a hard failure in check.sh.
            c["bone_influences"] = []
            for attr in ("positions",):
                for a in c.get(attr) or []:
                    a["data"]["Vector3"] = [xform_point(v, m) for v in a["data"]["Vector3"]]
            # Normals and tangents rotate but must not translate.
            for attr in ("normals", "tangents", "binormals"):
                for a in c.get(attr) or []:
                    key = "Vector4" if "Vector4" in a["data"] else "Vector3"
                    if key == "Vector4":
                        a["data"]["Vector4"] = [xform_dir(v[:3], m) + [v[3]]
                                                for v in a["data"]["Vector4"]]
                    else:
                        a["data"]["Vector3"] = [xform_dir(v, m) for v in a["data"]["Vector3"]]
            d["numshb"]["objects"].append(c)
        nvert += len(o["positions"][0]["data"]["Vector3"])
        lbl = mat_of.get(o["subindex"])
        for lst in (d["numdlb"]["entries"], d["nusrcmdlb"]["entries"]):
            for nm in (NAME, LEFT):
                lst.append({"mesh_object_name": nm,
                            "mesh_object_subindex": o["subindex"],
                            "material_label": lbl})

    # One bounding sphere per mesh object, recomputed in the BAKED frame -- the donor's own sphere
    # is in its model frame and would cull the mesh at the wrong place.
    pts = []
    for o in subs:
        pts += [xform_point(v, m) for v in o["positions"][0]["data"]["Vector3"]]
    cx = sum(p[0] for p in pts) / len(pts)
    cy = sum(p[1] for p in pts) / len(pts)
    cz = sum(p[2] for p in pts) / len(pts)
    r = max(((p[0] - cx) ** 2 + (p[1] - cy) ** 2 + (p[2] - cz) ** 2) ** 0.5 for p in pts)
    flags = [{"draw_model": True, "cast_shadow": True} for _ in subs]
    for nm in (NAME, LEFT):
        d["numshexb"]["mesh_object_groups"].append({
            "bounding_sphere": {"center": {"x": cx, "y": cy, "z": cz}, "radius": r},
            "mesh_object_full_name": nm, "mesh_object_name": nm, "entry_flags": flags})

    save(d)
    print("  added %s + %s: %d subindex/es, %d vertices, %d material(s)"
          % (NAME, LEFT, len(subs), nvert, len(set(mat_of.values()))))
    print("        baked by inverse(world_esword(HaveR)); sphere c=(%.3f,%.3f,%.3f) r=%.4f"
          % (cx, cy, cz, r))

    # Textures the new materials name, shipped beside the model. `#replace_cubemap` is an engine
    # placeholder, not a file.
    want = set()
    for e in d["numatb"]["entries"]:
        if e["material_label"] in set(mat_of.values()):
            for t in e.get("textures") or []:
                if not t["data"].startswith("#"):
                    want.add(t["data"])
    n = 0
    for t in sorted(want):
        dst = os.path.join(MODEL, t + ".nutexb")
        if os.path.exists(dst):
            continue
        src = None
        for r in SRC_ROOTS:
            if os.path.exists(os.path.join(r, t + ".nutexb")):
                src = os.path.join(r, t + ".nutexb")
                break
        src = src or os.path.join(SRC, t + ".nutexb")
        if os.path.exists(src):
            shutil.copyfile(src, dst)
            n += 1
        else:
            print("  !! texture %s not in %s -- pull it with tools/arc-fetch.sh" % (t, SRC))
    # COUNTED, NOT INFERRED. This line used to report `len(want) - n` as "already present",
    # which silently relabelled MISSING textures as fine: the first run printed eleven `!!` lines
    # and then "0 copied, 11 already present" in the same breath.
    here = sum(1 for t in want if os.path.exists(os.path.join(MODEL, t + ".nutexb")))
    print("        textures: %d copied, %d of %d now beside the model" % (n, here, len(want)))
    if here != len(want):
        print("  !! %d texture(s) still missing -- the blade will render untextured"
              % (len(want) - here), file=sys.stderr)
        return 1
    return 0


def remove():
    d = load()
    if not any(o["name"] in (NAME, LEFT) for o in d["numshb"]["objects"]):
        print("  nothing to remove")
        return 0
    d["numshb"]["objects"] = [o for o in d["numshb"]["objects"] if o["name"] not in (NAME, LEFT)]
    for k in ("numdlb", "nusrcmdlb"):
        d[k]["entries"] = [e for e in d[k]["entries"] if e["mesh_object_name"] not in (NAME, LEFT)]
    d["numshexb"]["mesh_object_groups"] = [g for g in d["numshexb"]["mesh_object_groups"]
                                           if g["mesh_object_full_name"] not in (NAME, LEFT)]
    save(d)
    print("  removed %s and %s" % (NAME, LEFT))
    return 0


def verify():
    d = load()
    bad = []
    by = {}
    for o in d["numshb"]["objects"]:
        by.setdefault(o["name"], []).append(o)
    for nm, hand in ((NAME, HAND), (LEFT, HAND_L)):
        if nm not in by:
            bad.append("%s is missing" % nm)
            continue
        for o in by[nm]:
            if o["parent_bone_name"] != hand:
                bad.append("%s binds to %s, expected %s" % (nm, o["parent_bone_name"], hand))
            if o.get("bone_influences"):
                bad.append("%s is still skinned (%d influence bone(s)); it must be single-bound"
                           % (nm, len(o["bone_influences"])))
    if NAME in by and LEFT in by and len(by[NAME]) != len(by[LEFT]):
        bad.append("%s has %d subindex/es, %s has %d"
                   % (NAME, len(by[NAME]), LEFT, len(by[LEFT])))
    labels = {e["material_label"] for e in d["numatb"]["entries"]}
    for e in d["numdlb"]["entries"]:
        if e["mesh_object_name"] in (NAME, LEFT) and e["material_label"] not in labels:
            bad.append("%s names material %s, which is not in the numatb"
                       % (e["mesh_object_name"], e["material_label"]))
    for b in bad:
        print("  FAIL " + b, file=sys.stderr)
    if bad:
        return 1
    n = sum(len(o["positions"][0]["data"]["Vector3"]) for o in by[NAME])
    print("  ok  %s %d subindex/es, %d vertices, single-bound to %s, with a %s twin"
          % (NAME, len(by[NAME]), n, HAND, LEFT))
    return 0


if __name__ == "__main__":
    a = sys.argv[1] if len(sys.argv) > 1 else ""
    sys.exit(verify() if a == "--verify" else remove() if a == "--remove" else add())
