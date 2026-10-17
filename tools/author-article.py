#!/usr/bin/env python3
"""Builds an article model for a weapon the game does not ship as an article.

WHY THIS EXISTS. Three of the five weapons this moveset shows are real articles in the game and
can simply be cloned -- Chrom's sword, Cloud's Fusion Sword, Mythra's blade. Two are not, and no
amount of looking will make them so: Corrin's Yato is `Kamui_Weapon_VIS_O_OBJShape` inside
`kamui/model/body`, and Palutena's staff is `staff_group_VIS_O_OBJShape` inside
`palutena/model/body`. She has exactly three model directories -- `body`, `gate`, `godwing` -- and
there is no staff among them and no `WEAPON_KIND_PALUTENA_*` for one.

So they stayed merged meshes, and the merged mesh is the thing that does not work facing left. The
measurements are in docs/the-article-path.md; the short version is that the graft is
GEOMETRICALLY EXACT (0.000001 over 444 vertices against the native formula) and still wrong, while
`ArticleModule::have` on a real article put Cloud's sword in the hand correctly in both facings
with the probe's lateral offset reading 0.000 either way.

The way to give those two the same treatment is to AUTHOR the article model they never had, out of
the donor's own body model, and host it on a weapon kind that already exists. That is data work,
not a new mechanism, and every file involved round-trips losslessly through
`vendor/ssbh_lib/ssbh_data_json.exe` -- verified: dump, rewrite, dump again, JSON identical.

THE SKELETON IS TWO BONES, and it is the native formula written as data. Natively the staff renders
at `v . local(Stick) . world_palutena(HaveR)`, because the mesh is single-bound to `Stick` and
`Stick`'s parent is `HaveR`. An article whose skeleton is

    HaveR   identity, no parent      <- the socket the engine attaches
    Stick   local(Stick), parent 0   <- the bind bone, mesh single-bound to it

renders at `v . local(Stick) . W`, where `W` is whatever the engine aligns the root to. Point that
at Pikachu's `haver` and it is the same expression with his hand substituted for hers. Nothing is
baked, nothing is recomputed, and the vertices are copied across untouched.

Palutena's `Stick` turns out to be a pure translation of (0, -2.2, 0) from `HaveR` -- no rotation
at all -- which is why this is worth doing rather than fearing: there is no orientation to get
wrong.

HOSTED ON A WEAPON KIND THAT ALREADY WORKS. The article still has to BE something, so each recipe
names a host: a clean, script-less held weapon whose assets we replace. `WEAPON_KIND_GANON_SWORD`
is the staff's -- `lua2cpp_ganon` contains `ganon` and `ganon_ganond` and no `ganon_sword`, so
there is no donor code at all, and tools/check-article-donor-params.sh audits it CLEAN across all
114 vtable slots. The host's `motion/` directory and its `model.xmb`/`model.nuhlpb` are copied
verbatim so the engine finds everything it expects; only the geometry is ours.

    py tools/author-article.py --list
    py tools/author-article.py staff
"""
import json
import math
import os
import shutil
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
SSBH = os.path.join(ROOT, "vendor", "ssbh_lib", "ssbh_data_json.exe")
LIBJSON = os.path.join(ROOT, "vendor", "ssbh_lib", "ssbh_lib_json.exe")
MOD = os.path.join(ROOT, "pikachu-moveset", "mod", "fighter", "pikachu")
# Same two source roots, in the same order, as tools/gen-article-assets.sh.
SRC_ROOTS = [
    os.path.join(ROOT, "work", "arc-vis", "fighter"),
    os.path.join(ROOT, "extracted", "fighter"),
]

RECIPES = {
    # Palutena's staff. Her n-air and her grab.
    "staff": dict(
        host_kind="WEAPON_KIND_GANON_SWORD",
        host="ganon/sword",
        src="palutena/model/body",
        meshes=["staff_group_VIS_O_OBJShape"],
        bind_bone="Stick",
        socket="HaveR",
    ),
    # Corrin's Yato. His up-air. Rigid, no influences, single-bound -- the header of
    # src/sword/mod.rs has the history of this one being recorded as impossible for a year.
    # SOCKET `HaveR`, NOT `HandR`, AND THE SKELETON IS ONE BONE BECAUSE OF IT.
    #
    # The first version was `HandR`(identity) -> `HaveR`(Corrin's own local transform) with socket
    # `handr`, reproducing `v . local_kamui(HaveR) . world_pika(HandR)`. That is only equal to
    # Corrin's own `v . world_kamui(HaveR)` if Pikachu's HandR->HaveR step matches Corrin's, and it
    # does not -- they are different rigs.
    #
    # The playtest said so plainly: the article generated and sat at Pikachu's FEET, which is where
    # an article that exists but never attached ends up -- the owner's origin. `ArticleModule::have`
    # had refused `handr`. That is the third socket to be refused after `rot`, against fourteen
    # native call sites that all pass `haver`, so `haver` is treated here as the only socket `have`
    # accepts rather than as one option among several.
    #
    # One identity bone at `HaveR` with the mesh bound to it renders at `v . world_pika(HaveR)`,
    # which is the direct analogue of Corrin's own single bind, and needs no second bone at all.
    "yato": dict(
        host_kind="WEAPON_KIND_GAMEWATCH_NORMAL_WEAPON",
        host="gamewatch/normal_weapon",
        src="kamui/model/body",
        meshes=["Kamui_Weapon_VIS_O_OBJShape"],
        bind_bone="HaveR",
        socket="HaveR",
    ),
}


def find_src(rel):
    for r in SRC_ROOTS:
        p = os.path.join(r, rel)
        if os.path.isdir(p) and os.listdir(p):
            return p
    return None


def dump(tool, path, out):
    subprocess.run([tool, path, out], check=True, capture_output=True)
    with open(out, encoding="utf-8") as f:
        return json.load(f)


def write(tool, obj, jpath, out):
    with open(jpath, "w", encoding="utf-8") as f:
        json.dump(obj, f, indent=2)
    subprocess.run([tool, jpath, out], check=True, capture_output=True)


def verts(o):
    d = o["positions"][0]["data"]
    return d[list(d.keys())[0]]


def sphere(vs):
    """Bounding sphere in the mesh's own space, the same convention the game's files use.

    Centre is the bounding-box midpoint and the radius is the furthest vertex from it -- checked
    against the shipped body model, where recomputing this way reproduced all five weapons'
    spheres and contained every vertex.
    """
    mn = [min(v[i] for v in vs) for i in range(3)]
    mx = [max(v[i] for v in vs) for i in range(3)]
    c = [(mn[i] + mx[i]) / 2 for i in range(3)]
    r = max(math.dist(v, c) for v in vs)
    return c, r


def identity():
    return [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]


def mat_mul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(4)) for j in range(4)] for i in range(4)]


def world_bind(bones, index):
    """Accumulate a bone's local transforms up the parent chain to its bind-pose world matrix."""
    m = bones[index]["transform"]
    p = bones[index].get("parent_index")
    while p is not None:
        m = mat_mul(m, bones[p]["transform"])
        p = bones[p].get("parent_index")
    return m


# THE DERIVED ROOT IS BEHIND A SWITCH, because it is an ASSET change -- it rewrites two article
# skeletons that the engine loads during match init, which is exactly where four consecutive boots
# went wrong on 2026-09-18. Part 11's evidence for it is strong and is unaffected by this; what is
# unproven is that it is safe to load, and the honest way to find out is one boot with it as the
# only variable. `ARTICLE_ROOT_ROTATION=1` turns it back on.
DERIVE_ROOT = os.environ.get("ARTICLE_ROOT_ROTATION", "0") == "1"


def socket_bind_rotation(socket, work):
    """The HOST's socket-joint bind ROTATION, which is what a held article's root bone carries.

    DERIVED FROM THE GAME, NOT CHOSEN. Dump the root bone of the two articles the game actually
    holds in a hand and compare each against its OWNER's `HaveR` bind rotation:

        chrom/model/sword        root [[-0.185,-0.983, 0.014],[-0.494, 0.106, 0.863],[-0.85, 0.152,-0.505]]
        chrom  HaveR bind rot         [[-0.185,-0.983, 0.014],[-0.494, 0.106, 0.863],[-0.85, 0.152,-0.505]]

        cloud/model/fusionsword  root [[0,-1,0],[0,0,1],[-1,0,0]]
        cloud  HaveR bind rot         [[0,-1,0],[0,0,1],[-1,0,0]]

    Both match to three decimals, and Chrom's is not axis-aligned, so this is not a coincidence:
    **a held article's root bone carries its owner's socket-joint bind rotation.**

    It also explains the symptom that prompted the search. `ArticleModule::have` appears to supply
    position and the owner's FACING but not the joint's orientation -- so the root bone is what
    supplies the hand's orientation, and an authored article with an IDENTITY root leaves its mesh
    in raw model space. The playtest called that "stuck inside pikachu's body facing up".

    The owner here is Pikachu, so it is HIS bind rotation that goes in, not the donor's: it puts
    the donor's haver-local mesh into Pikachu's hand frame the same way it sits in the donor's.
    Translation stays zero -- `have` provides the position.
    """
    # THE MOD'S OWN SKELETON, not a donor root: this is the file the game actually loads for
    # Pikachu, and it is the one the graft reparented `Mouth` in. `find_src` only searches the
    # donor roots and has no Pikachu body model to find.
    skel = os.path.join(MOD, "model", "body", "c00", "model.nusktb")
    if not os.path.exists(skel):
        sys.exit("  !! %s is missing; cannot derive the socket bind rotation" % skel)
    bones = dump(SSBH, skel, os.path.join(work, "host.nusktb.json"))["bones"]
    by = {b["name"].lower(): i for i, b in enumerate(bones)}
    if socket.lower() not in by:
        sys.exit("  !! pikachu has no bone %s to take a bind rotation from" % socket)
    w = world_bind(bones, by[socket.lower()])
    return [[w[0][0], w[0][1], w[0][2], 0.0],
            [w[1][0], w[1][1], w[1][2], 0.0],
            [w[2][0], w[2][1], w[2][2], 0.0],
            [0.0, 0.0, 0.0, 1.0]]


def author(name, work):
    r = RECIPES[name]
    src = find_src(r["src"])
    if src is None:
        sys.exit("  !! %s is in neither work/arc-vis nor extracted -- pull it with arc-fetch" % r["src"])
    src = os.path.join(src, "c00")
    hf, ha = r["host"].split("/")
    host_model = find_src("%s/model/%s" % (hf, ha))
    host_motion = find_src("%s/motion/%s" % (hf, ha))
    if not host_model or not host_motion:
        sys.exit("  !! host %s model/motion not pulled" % r["host"])
    host_model = os.path.join(host_model, "c00")
    host_motion = os.path.join(host_motion, "c00")

    # ARTICLE_SLOTS IS HONOURED HERE TOO, and hardcoding c00 was a footgun worth removing before
    # anyone hit it: tools/gen-article-assets.sh reads the same variable for the COPIED articles,
    # so a release built with ARTICLE_SLOTS=all would have shipped eight slots of every cloned
    # article and one slot of the two authored ones -- and a cNN Pikachu resolves an article under
    # his OWN costume slot, so the staff and the Yato would simply not have been there.
    #
    # The authored model is identical for every slot (it is built from the donor's c00 body model,
    # and Pikachu's costume does not change Palutena's staff), so the first slot is built and the
    # rest are copied from it. One directory copy each, for the reason in mirror-costumes.sh.
    slots = os.environ.get("ARTICLE_SLOTS", "c00")
    slots = ["c%02d" % i for i in range(8)] if slots == "all" else slots.split()
    dst_model = os.path.join(MOD, "model", name, slots[0])
    dst_motion = os.path.join(MOD, "motion", name, slots[0])
    for d in (dst_model, dst_motion):
        shutil.rmtree(os.path.dirname(d), ignore_errors=True)
        os.makedirs(d, exist_ok=True)

    # ---- skeleton: socket (identity) -> bind bone (its own local transform) --------------------
    sk = dump(SSBH, os.path.join(src, "model.nusktb"), os.path.join(work, "src.nusktb.json"))
    by_name = {b["name"]: (i, b) for i, b in enumerate(sk["bones"])}
    if r["bind_bone"] not in by_name:
        sys.exit("  !! %s has no bone %s" % (r["src"], r["bind_bone"]))
    bi, bone = by_name[r["bind_bone"]]
    parent = sk["bones"][bone["parent_index"]]["name"] if bone["parent_index"] is not None else None
    # ONE BONE WHEN THE BIND BONE *IS* THE SOCKET, TWO WHEN IT HANGS OFF IT.
    #
    # The two-bone form exists because Palutena's staff binds to `Stick`, a child of `HaveR`: the
    # article has to carry `Stick`'s own local transform so the mesh lands where hers does. When
    # the donor's mesh binds directly to the joint the engine will attach -- Corrin's Yato binds to
    # HIS `HaveR`, and `have` only accepts `haver` -- there is no intermediate transform to carry,
    # and a single identity bone reproduces `v . world(HaveR)` exactly.
    #
    # Getting this wrong is what put the Yato on Pikachu's feet: the first version declared socket
    # `HandR` so it could keep the two-bone shape, and `have` refuses `handr`.
    if r["bind_bone"] == r["socket"]:
        # The root carries the HOST's socket bind rotation -- see socket_bind_rotation. An identity
        # root here is what left the Yato "inside pikachu's body facing up".
        out_sk = dict(major_version=1, minor_version=0, bones=[
            dict(name=r["socket"],
                 transform=socket_bind_rotation(r["socket"], work) if DERIVE_ROOT else identity(),
                 parent_index=None, billboard_type="Disabled"),
        ])
    else:
        if parent != r["socket"]:
            sys.exit("  !! %s's parent is %s, not the declared socket %s -- the two-bone form only "
                     "reproduces the native transform when the bind bone hangs directly off the "
                     "socket" % (r["bind_bone"], parent, r["socket"]))
        # THE ROOT CARRIES THE HOST BIND ROTATION IN THIS FORM TOO. The argument is the same one
        # in socket_bind_rotation and it does not depend on the bone count: the staff renders at
        # `v . local(Stick) . W(HaveR)` on Palutena, so reproducing its orientation RELATIVE TO
        # THE HAND on Pikachu means the root must play the part of `W(HaveR)` -- his, not hers.
        # An identity root here would leave the staff in raw model space exactly as it left the
        # Yato "inside pikachu's body facing up".
        out_sk = dict(major_version=1, minor_version=0, bones=[
            dict(name=r["socket"],
                 transform=socket_bind_rotation(r["socket"], work) if DERIVE_ROOT else identity(),
                 parent_index=None, billboard_type="Disabled"),
            dict(name=r["bind_bone"], transform=bone["transform"], parent_index=0,
                 billboard_type="Disabled"),
        ])
    write(SSBH, out_sk, os.path.join(work, "out.nusktb.json"),
          os.path.join(dst_model, "model.nusktb"))

    # ---- mesh: the donor's objects, copied untouched -------------------------------------------
    sh = dump(SSBH, os.path.join(src, "model.numshb"), os.path.join(work, "src.numshb.json"))
    keep = [o for o in sh["objects"] if o["name"] in r["meshes"]]
    if not keep:
        sys.exit("  !! none of %s found in %s/model.numshb" % (r["meshes"], r["src"]))
    for o in keep:
        if o["parent_bone_name"] != r["bind_bone"]:
            sys.exit("  !! %s sub %d is bound to %r, not %s -- this recipe only handles a mesh "
                     "single-bound to the bind bone"
                     % (o["name"], o["subindex"], o["parent_bone_name"], r["bind_bone"]))
        if o.get("bone_influences"):
            sys.exit("  !! %s sub %d is SKINNED (%d influence bones). A skinned mesh needs its "
                     "influence bones in the article skeleton too, which this recipe does not do."
                     % (o["name"], o["subindex"], len(o["bone_influences"])))
    out_sh = dict(major_version=sh["major_version"], minor_version=sh["minor_version"], objects=keep)
    write(SSBH, out_sh, os.path.join(work, "out.numshb.json"),
          os.path.join(dst_model, "model.numshb"))

    # ---- modl: mesh/subindex -> material, taken from the donor's own mapping -------------------
    md = dump(SSBH, os.path.join(src, "model.numdlb"), os.path.join(work, "src.numdlb.json"))
    want = {(o["name"], o["subindex"]) for o in keep}
    entries = [e for e in md["entries"]
               if (e["mesh_object_name"], e["mesh_object_subindex"]) in want]
    if len(entries) != len(keep):
        sys.exit("  !! %d modl entries for %d mesh objects" % (len(entries), len(keep)))
    out_md = dict(major_version=md["major_version"], minor_version=md["minor_version"],
                  model_name="model", skeleton_file_name="model.nusktb",
                  material_file_names=["model.numatb"], animation_file_name=None,
                  mesh_file_name="model.numshb", entries=entries)
    for f in ("model.numdlb", "model.nusrcmdlb"):
        # nusrcmdlb is byte-identical to numdlb in every fighter model checked, so it is written
        # from the same object rather than copied -- see docs/the-article-path.md.
        write(SSBH, out_md, os.path.join(work, "out.numdlb.json"), os.path.join(dst_model, f))

    # ---- materials: only the ones those meshes reference ---------------------------------------
    labels = {e["material_label"] for e in entries}
    mt = dump(LIBJSON, os.path.join(src, "model.numatb"), os.path.join(work, "src.numatb.json"))
    ver = list(mt["Matl"].keys())[0]
    all_e = mt["Matl"][ver]["entries"]
    kept = [e for e in all_e if e["material_label"] in labels]
    if len(kept) != len(labels):
        sys.exit("  !! wanted materials %s, found %d" % (sorted(labels), len(kept)))
    mt["Matl"][ver]["entries"] = kept
    write(LIBJSON, mt, os.path.join(work, "out.numatb.json"),
          os.path.join(dst_model, "model.numatb"))

    # ---- textures those materials reference ----------------------------------------------------
    tex = set()
    for e in kept:
        for a in e["attributes"]:
            data = a["param"]["data"]
            if "String" in data:
                t = data["String"]
                if not t.startswith("#"):
                    tex.add(t)
    # ENGINE-COMMON TEXTURES ARE NOT SHIPPED, and must not be. `default_normal` and friends live
    # at `common/shader/sfxpbs/` -- 308 paths in the arc index reference that name and only a
    # handful of fighters carry a local copy -- so the loader resolves them without the model
    # directory having one. Corrin's Yato material references `default_Normal`, which is why this
    # distinction exists rather than being a tidy-up.
    missing = []
    for t in sorted(tex):
        p = os.path.join(src, t + ".nutexb")
        if os.path.isfile(p):
            shutil.copy2(p, os.path.join(dst_model, t + ".nutexb"))
        elif t.lower().startswith("default_"):
            pass
        else:
            missing.append(t)

    # ---- mesh ex: one group per mesh NAME, flags per subindex ----------------------------------
    groups = []
    for nm in r["meshes"]:
        subs = [o for o in keep if o["name"] == nm]
        if not subs:
            continue
        vs = [v for o in subs for v in verts(o)]
        c, rad = sphere(vs)
        groups.append(dict(
            bounding_sphere=dict(center=dict(x=c[0], y=c[1], z=c[2]), radius=rad),
            mesh_object_full_name=nm,
            # The game strips the `_VIS_O_OBJ...` suffix for the short name; every vanilla group
            # follows that, and the merged body model's five weapons are the only ones that did
            # not because they were given plain names.
            mesh_object_name=nm.split("_VIS_")[0],
            entry_flags=[dict(draw_model=True, cast_shadow=True) for _ in subs],
        ))
    write(SSBH, dict(mesh_object_groups=groups), os.path.join(work, "out.numshexb.json"),
          os.path.join(dst_model, "model.numshexb"))

    # ---- everything the engine expects but we do not author ------------------------------------
    # xmb is per-MODEL settings and shared byte-identically across unrelated models; nuhlpb is the
    # host's helper bones, which a two-bone skeleton does not use but the loader may still open.
    for f in ("model.xmb", "model.nuhlpb"):
        p = os.path.join(host_model, f)
        if os.path.isfile(p):
            shutil.copy2(p, os.path.join(dst_model, f))
    for f in sorted(os.listdir(host_motion)):
        p = os.path.join(host_motion, f)
        if os.path.isfile(p):
            shutil.copy2(p, os.path.join(dst_motion, f))

    n_model = len(os.listdir(dst_model))
    n_motion = len(os.listdir(dst_motion))
    for extra in slots[1:]:
        for base, built in (("model", dst_model), ("motion", dst_motion)):
            tgt = os.path.join(MOD, base, name, extra)
            shutil.rmtree(tgt, ignore_errors=True)
            shutil.copytree(built, tgt)
    print("  authored %-6s from %s %s -> %d model + %d motion file(s) x %d slot(s), host %s"
          % (name, r["src"], r["meshes"], n_model, n_motion, len(slots), r["host_kind"]))
    rootdesc = "host bind rotation" if DERIVE_ROOT else "identity"
    shape = ("%s(%s), single bind" % (r["socket"], rootdesc)
             if r["bind_bone"] == r["socket"]
             else "%s(%s) -> %s" % (r["socket"], rootdesc, r["bind_bone"]))
    print("           skeleton %s%s"
          % (shape,
             "" if not missing else "   !! textures not in the source dir: %s" % missing))
    return not missing


def row_enabled(name):
    """Whether this recipe's Candidate row in src/articles/mod.rs is enabled.

    `clone_enabled()` covers the global switch; this covers the PER-ROW one, and the difference
    cost a deploy. gen-article-assets.sh removes an article's assets when its row is disabled, so
    verifying a disabled row finds an absent directory and fails a tree that is exactly right --
    the same shape of bug as the global case, one level down. Found on 2026-09-19 when `staff`,
    `esword` and `yato` were disabled in favour of the dual-wielded merged mesh.
    """
    src = os.path.join(ROOT, "pikachu-moveset", "plugin", "src", "articles", "mod.rs")
    try:
        text = open(src, encoding="utf-8").read()
    except OSError:
        return True
    for line in text.splitlines():
        if 'host_name: "%s"' % name in line:
            return "enabled: true" in line
    return True


def clone_enabled():
    """Whether src/articles/mod.rs has cloning switched on.

    `--verify` has to honour this for the same reason gen-article-assets.sh does: with cloning off
    that script REMOVES every article's assets, so verifying them finds an absent directory and
    reports a failure for a tree that is exactly correct. Found when articles were disabled as a
    bisect on 2026-09-19 and gate 3f2 failed on a build whose only problem was the gate.
    """
    src = os.path.join(ROOT, "pikachu-moveset", "plugin", "src", "articles", "mod.rs")
    try:
        return "pub const CLONE_ENABLED: bool = true;" in open(src, encoding="utf-8").read()
    except OSError:
        return True


def verify(name, work):
    """Re-derive the recipe and check the SHIPPED files against it.

    Authored articles cannot be checked the way copied ones are -- tools/check.sh 3f asserts every
    article file is byte-identical to the arc's own copy, and half of these files have no arc copy
    to be identical to. So the invariants are checked instead, and they are the ones that decide
    whether the thing renders in the right place:
      * the skeleton is exactly socket(identity) -> bind bone, with the bind bone carrying the
        donor's own local transform to the last bit
      * every vertex is bit-identical to the donor's body model
      * the bounding sphere contains every vertex
      * every mesh/subindex has a modl entry, and every material it names is in the numatb
      * every texture those materials name is either shipped or engine-common
    """
    r = RECIPES[name]
    src = find_src(r["src"])
    if src is None:
        print("  skip  %s not pulled, cannot verify %s" % (r["src"], name)); return True
    src = os.path.join(src, "c00")
    slots = os.environ.get("ARTICLE_SLOTS", "c00")
    slots = ["c%02d" % i for i in range(8)] if slots == "all" else slots.split()
    d = os.path.join(MOD, "model", name, slots[0])
    if not os.path.isdir(d):
        print("  FAIL %s is enabled and authored but %s does not exist" % (name, d)); return False
    bad = []

    sk = dump(SSBH, os.path.join(d, "model.nusktb"), os.path.join(work, "v.nusktb.json"))
    ssk = dump(SSBH, os.path.join(src, "model.nusktb"), os.path.join(work, "vs.nusktb.json"))
    sbone = [b for b in ssk["bones"] if b["name"] == r["bind_bone"]][0]
    # ONE bone when the bind bone IS the socket, TWO when it hangs off it. See the builder.
    want_bones = 1 if r["bind_bone"] == r["socket"] else 2
    if len(sk["bones"]) != want_bones:
        bad.append("skeleton has %d bones, expected %d" % (len(sk["bones"]), want_bones))
    else:
        a = sk["bones"][0]
        if a["name"] != r["socket"] or a["parent_index"] is not None:
            bad.append("root is %r parent=%s, expected %r with no parent"
                       % (a["name"], a["parent_index"], r["socket"]))
        # TOLERANCE, NOT EQUALITY. The matrix survives a JSON -> nusktb -> JSON round trip as
        # f32, so an exact list compare fails on the last decimal place of a value that is
        # otherwise correct. 1e-5 is far tighter than any error that would matter on screen and
        # far looser than f32 rounding.
        want_root = socket_bind_rotation(r["socket"], work) if DERIVE_ROOT else identity()
        if any(abs(a["transform"][i][j] - want_root[i][j]) > 1e-5
               for i in range(4) for j in range(4)):
            bad.append("root transform is not the host socket bind rotation")
        if want_bones == 2:
            b = sk["bones"][1]
            if b["name"] != r["bind_bone"] or b["parent_index"] != 0:
                bad.append("child is %r parent=%s, expected %r parent 0"
                           % (b["name"], b["parent_index"], r["bind_bone"]))
            if b["transform"] != sbone["transform"]:
                bad.append("bind bone transform does not match the donor's own")

    sh = dump(SSBH, os.path.join(d, "model.numshb"), os.path.join(work, "v.numshb.json"))
    ssh = dump(SSBH, os.path.join(src, "model.numshb"), os.path.join(work, "vs.numshb.json"))
    s_by = {(o["name"], o["subindex"]): o for o in ssh["objects"]}
    nvert = 0
    for o in sh["objects"]:
        k = (o["name"], o["subindex"])
        if k not in s_by:
            bad.append("%s sub %d is not in the donor's body model" % k); continue
        av, sv = verts(o), verts(s_by[k])
        if av != sv:
            bad.append("%s sub %d vertices differ from the donor's" % k)
        nvert += len(av)
        if o["parent_bone_name"] != r["bind_bone"]:
            bad.append("%s sub %d binds to %r, not %s" % (k[0], k[1], o["parent_bone_name"], r["bind_bone"]))

    ex = dump(SSBH, os.path.join(d, "model.numshexb"), os.path.join(work, "v.numshexb.json"))
    for g in ex["mesh_object_groups"]:
        subs = [o for o in sh["objects"] if o["name"] == g["mesh_object_full_name"]]
        c = g["bounding_sphere"]["center"]
        rad = g["bounding_sphere"]["radius"]
        worst = max(math.dist(v, (c["x"], c["y"], c["z"])) for o in subs for v in verts(o))
        if worst > rad + 1e-3:
            bad.append("%s sphere misses by %.4f" % (g["mesh_object_full_name"], worst - rad))
        if len(g["entry_flags"]) != len(subs):
            bad.append("%s has %d entry_flags for %d subindices"
                       % (g["mesh_object_full_name"], len(g["entry_flags"]), len(subs)))

    md = dump(SSBH, os.path.join(d, "model.numdlb"), os.path.join(work, "v.numdlb.json"))
    mt = dump(LIBJSON, os.path.join(d, "model.numatb"), os.path.join(work, "v.numatb.json"))
    have_mat = {e["material_label"] for e in mt["Matl"][list(mt["Matl"].keys())[0]]["entries"]}
    modl = {(e["mesh_object_name"], e["mesh_object_subindex"]): e["material_label"]
            for e in md["entries"]}
    for o in sh["objects"]:
        k = (o["name"], o["subindex"])
        if k not in modl:
            bad.append("%s sub %d has no modl entry" % k)
        elif modl[k] not in have_mat:
            bad.append("%s sub %d names material %r which is not in the numatb" % (k[0], k[1], modl[k]))
    for e in mt["Matl"][list(mt["Matl"].keys())[0]]["entries"]:
        for a in e["attributes"]:
            data = a["param"]["data"]
            if "String" in data:
                t = data["String"]
                if t.startswith("#") or t.lower().startswith("default_"):
                    continue
                if not os.path.isfile(os.path.join(d, t + ".nutexb")):
                    bad.append("material %s references texture %s which is not shipped"
                               % (e["material_label"], t))
    if not os.path.isfile(os.path.join(d, "model.nusrcmdlb")):
        bad.append("model.nusrcmdlb is missing")

    if bad:
        print("  FAIL %s:" % name)
        for x in bad:
            print("       %s" % x)
        return False
    print("  ok  %-6s %d bone(s), %d mesh object(s), %d vertices, all bit-identical to %s"
          % (name, len(sk["bones"]), len(sh["objects"]), nvert, r["src"]))
    return True


def main():
    if len(sys.argv) < 2 or sys.argv[1] == "--list":
        print(" ".join(sorted(RECIPES)))
        return 0
    if sys.argv[1] == "--verify":
        if not clone_enabled():
            print("  -   clone disabled in src/articles/mod.rs; authored assets are intentionally "
                  "absent, nothing to verify")
            return 0
        work = os.path.join(ROOT, "work", "authored")
        os.makedirs(work, exist_ok=True)
        names = sys.argv[2:] or sorted(RECIPES)
        skipped = [n for n in names if not row_enabled(n)]
        names = [n for n in names if row_enabled(n)]
        for n in skipped:
            print("  -   %-6s row is disabled; its assets are intentionally absent" % n)
        return 0 if all(verify(n, work) for n in names) else 1
    work = os.path.join(ROOT, "work", "authored")
    os.makedirs(work, exist_ok=True)
    ok = True
    for name in sys.argv[1:]:
        if name not in RECIPES:
            sys.exit("  !! no recipe named %r (have: %s)" % (name, " ".join(sorted(RECIPES))))
        ok = author(name, work) and ok
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
