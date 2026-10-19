#!/usr/bin/env python3
"""One-slots a mod's fighter effects on the console, in place, over FTP.

    python tools/oneslot-eff.py "<mod folder>" <fighter> <slot>[,<slot>...]
    python tools/oneslot-eff.py --dry-run "Shrek over Ganondorf" ganon 0

WHAT ONE-SLOTTING IS, from the One Slot Effects readme (Documents/switch/one_slot_eff_13-0-4,
"Effect Slotter Script (for modders)/info.txt"), which is the authority for all of this:

    ef_<fighter>.eff   ->  ef_<fighter>_c<NN>.eff
    trail/             ->  trail_c<NN>/
    model/<name>/      ->  model/<name>_c<NN>/

and every file involved has to be declared in a `config.json` under `fighter/<fighter>/c<NN>`,
because those paths do not exist in the archive and nothing loads an undeclared path.

THE TRAP THE README CALLS OUT, and the reason this is a script rather than four renames: "when you
are one-slotting trails and models, you need to make sure that *every file* is added again, so even
if you are only changing one trail you need to put the original vanilla files for the files you
didn't touch in that directory". A mod that ships three of Cloud's eight trail textures and
declares only those three gets a slot with five missing trails.

THIS DOES IT WITHOUT COPYING ANY NINTENDO DATA. The readme says to put the vanilla files in; the
same effect is had by declaring them and pointing them at the copies already in the archive with
`share-to-added`, which is what that key is for and needs neither a game running to pull them nor a
redistributed texture. The vanilla file list comes from `work/acquire/arc-paths.txt`, the real
archive map.

RENAME, NEVER DELETE. Every change to the card is an FTP RNFR/RNTO, so undoing it is renaming back;
nothing is removed and nothing is re-uploaded except `config.json`. An existing `config.json` is
downloaded and MERGED, never clobbered, and a copy of the original is kept under `work/oneslot/`.

MULTIPLE SLOTS: the first is a rename and the rest are copies, because one file cannot be at four
paths. Passing `1,3,5,7` gives a mod that covers exactly those costumes, which is what a skin
shipping four alts needs.
"""
import json
import os
import re
import subprocess
import sys
import urllib.parse

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ARCMAP = os.path.join(ROOT, "work", "acquire", "arc-paths.txt")
BACKUP = os.path.join(ROOT, "work", "oneslot")
def console_host():
    """Same rule as tools/console.sh: FTP_HOST, then .console-host.local, else stop."""
    h = os.environ.get("FTP_HOST", "").strip()
    if not h and os.path.isfile(os.path.join(ROOT, ".console-host.local")):
        h = open(os.path.join(ROOT, ".console-host.local"), encoding="utf-8").read().strip()
    if not h:
        sys.exit("  no console set: write its address to .console-host.local, or pass FTP_HOST=...")
    return h


HOST = console_host()
PORT = os.environ.get("FTP_PORT", "5000")
BASE = "/ultimate/mods"


def profile():
    """Login and card root, the way tools/console.sh works them out and caches them in
    work/.ftp-profile: sys-ftpd logs in as poaga with the card at `/`; sphaira (from 2026-10-18)
    is anonymous with the card at `/sdmc:`. Environment first, then that cache, then sys-ftpd's.
    Run any shell tool once (tools/ftpq.sh list /) to write the cache for a new server."""
    user = os.environ.get("FTP_USER")
    root = os.environ.get("FTP_ROOT")
    cache = os.path.join(ROOT, "work", ".ftp-profile")
    if (user is None or root is None) and os.path.isfile(cache):
        lines = open(cache, encoding="utf-8").read().split("\n")
        if len(lines) >= 3 and lines[0] == "%s:%s" % (HOST, PORT):
            user = user if user is not None else lines[1]
            root = root if root is not None else lines[2]
    return user or "poaga", root or ""


def password():
    with open(os.path.join(ROOT, ".ftp-credentials.local"), encoding="utf-8") as fh:
        return fh.read().strip()


USER, FTP_ROOT = profile()
PASS = "" if USER == "anonymous" else password()
DRY = False


def url(path):
    return "ftp://%s:%s%s%s" % (HOST, PORT, FTP_ROOT, urllib.parse.quote(path))


def curl(args, binary=False):
    cmd = ["curl", "-s", "-u", "%s:%s" % (USER, PASS), "--max-time", "60"] + args
    r = subprocess.run(cmd, capture_output=True)
    return r.stdout if binary else r.stdout.decode("utf-8", "replace")


def listdir(path):
    """(names, is_dir) for one remote directory."""
    out = curl([url(path + "/")])
    rows = []
    for line in out.replace("\r", "").split("\n"):
        parts = line.split(None, 8)
        if len(parts) < 9:
            continue
        rows.append((parts[8], parts[0].startswith("d")))
    return rows


def rename(src, dst):
    print("    rename %s -> %s" % (src.split("/")[-1], dst.split("/")[-1]))
    if DRY:
        return True
    r = subprocess.run(
        ["curl", "-s", "-u", "%s:%s" % (USER, PASS), "--max-time", "60",
         url(BASE + "/"), "-Q", "RNFR " + FTP_ROOT + src, "-Q", "RNTO " + FTP_ROOT + dst, "-o", os.devnull],
        capture_output=True)
    return r.returncode == 0


def mkdir(path):
    """Create one remote directory, ignoring "already exists".

    FTP UPLOAD DOES NOT CREATE DIRECTORIES, and finding that out cost a run: the copied
    `ef_cloud_c03.eff` landed because its parent existed, and all six of its trail textures
    silently did not because `trail_c03/` did not. The listing afterwards showed four .eff files
    and one trail directory, which is a mod that one-slots the effects and loses the sword trails
    on three of its four costumes.
    """
    if DRY:
        return
    subprocess.run(
        ["curl", "-s", "-u", "%s:%s" % (USER, PASS), "--max-time", "30",
         url(BASE + "/"), "-Q", "MKD " + FTP_ROOT + path, "-o", os.devnull], capture_output=True)


def upload(local, remote):
    print("    upload %s" % remote.split("/")[-1])
    if DRY:
        return True
    r = subprocess.run(
        ["curl", "-s", "-u", "%s:%s" % (USER, PASS), "--max-time", "120",
         "-T", local, url(remote)], capture_output=True)
    return r.returncode == 0


def main():
    global DRY
    args = [a for a in sys.argv[1:] if a != "--dry-run"]
    DRY = "--dry-run" in sys.argv
    if len(args) != 3:
        sys.stderr.write(__doc__)
        sys.exit(2)
    mod, fighter, slots_arg = args
    slots = [int(s) for s in slots_arg.split(",")]
    modroot = "%s/%s" % (BASE, mod)
    effdir = "effect/fighter/%s" % fighter

    if not os.path.isfile(ARCMAP):
        sys.exit("  no archive map at work/acquire/arc-paths.txt -- needed for the vanilla files")
    arc = set(open(ARCMAP, encoding="utf-8", errors="replace").read().split("\n"))

    present = listdir("%s/%s" % (modroot, effdir))
    if not present:
        sys.exit("  %s has no %s" % (mod, effdir))
    names = {n: d for n, d in present}
    print("  %s: %s holds %s" % (mod, effdir, ", ".join(sorted(names))))

    # ---- what the mod ships inside trail/ (or an already-slotted trail_cNN/) ------------------
    trail_src = next((n for n in names if n == "trail" or n.startswith("trail_c")), None)
    shipped_trail = []
    if trail_src:
        shipped_trail = sorted(n for n, d in listdir("%s/%s/%s" % (modroot, effdir, trail_src)) if not d)
    eff_src = next((n for n in names if n.endswith(".eff")), None)
    if eff_src is None:
        sys.exit("  no .eff in %s" % effdir)
    # `"_c" in eff_src` was the first test here and it is wrong for any fighter whose name
    # contains it: `ef_cloud.eff` matched on the "_c" of "cloud" and the rename was skipped. The
    # suffix has to be anchored.
    already = re.search(r"_c\d\d\.eff$", eff_src) is not None
    if already:
        print("  already one-slotted (%s) -- not renaming the .eff" % eff_src)

    # ---- the FULL vanilla trail set, which is what must be declared --------------------------
    van_trail = sorted(p.split("/")[-1] for p in arc
                       if p.startswith("effect/fighter/%s/trail/" % fighter))
    missing = [t for t in van_trail if t not in shipped_trail]
    print("  trail: mod ships %d, vanilla has %d, %d to share from vanilla"
          % (len(shipped_trail), len(van_trail), len(missing)))

    new_dir_files = {}
    share = {}
    for i, slot in enumerate(slots):
        c = "c%02d" % slot
        key = "fighter/%s/%s" % (fighter, c)
        files = ["%s/ef_%s_%s.eff" % (effdir, fighter, c)]
        for t in sorted(set(shipped_trail) | set(van_trail)):
            files.append("%s/trail_%s/%s" % (effdir, c, t))
        new_dir_files[key] = files
        for t in missing:
            share.setdefault("effect/fighter/%s/trail/%s" % (fighter, t), []).append(
                "%s/trail_%s/%s" % (effdir, c, t))

        if i == 0:
            if not already:
                rename("%s/%s/%s" % (modroot, effdir, eff_src),
                       "%s/%s/ef_%s_%s.eff" % (modroot, effdir, fighter, c))
            if trail_src and trail_src != "trail_%s" % c:
                rename("%s/%s/%s" % (modroot, effdir, trail_src),
                       "%s/%s/trail_%s" % (modroot, effdir, c))
        else:
            # EVERY SLOT AFTER THE FIRST IS A COPY, because one file cannot be at four paths. The
            # source is the slot already renamed above, so this never touches the mod's original
            # layout and re-running is idempotent.
            first = "c%02d" % slots[0]
            pairs = [("ef_%s_%s.eff" % (fighter, first), "ef_%s_%s.eff" % (fighter, c))]
            for t in shipped_trail:
                pairs.append(("trail_%s/%s" % (first, t), "trail_%s/%s" % (c, t)))
            print("    slot %s: copying %d file(s) from %s" % (c, len(pairs), first))
            if shipped_trail:
                mkdir("%s/%s/trail_%s" % (modroot, effdir, c))
            for src, dst in pairs:
                if DRY:
                    continue
                blob = curl([url("%s/%s/%s" % (modroot, effdir, src))], binary=True)
                if not blob:
                    print("      FAILED to read %s -- skipping %s" % (src, dst))
                    continue
                tmp = os.path.join(BACKUP, "copy.bin")
                with open(tmp, "wb") as fh:
                    fh.write(blob)
                upload(tmp, "%s/%s/%s" % (modroot, effdir, dst))

    # ---- config.json, merged ------------------------------------------------------------------
    os.makedirs(BACKUP, exist_ok=True)
    existing = curl([url("%s/config.json" % modroot)])
    cfg = {}
    if existing.strip().startswith("{"):
        with open(os.path.join(BACKUP, "%s.config.json.bak" % mod.replace("/", "_")),
                  "w", encoding="utf-8") as fh:
            fh.write(existing)
        try:
            cfg = json.loads(existing)
            print("  merging into the mod's existing config.json (original saved under work/oneslot)")
        except ValueError:
            sys.exit("  the mod has a config.json this cannot parse; stopping rather than clobbering it")
    cfg.setdefault("new-dir-files", {}).update(new_dir_files)
    if share:
        for k, v in share.items():
            cfg.setdefault("share-to-added", {}).setdefault(k, [])
            for t in v:
                if t not in cfg["share-to-added"][k]:
                    cfg["share-to-added"][k].append(t)
    local = os.path.join(BACKUP, "%s.config.json" % mod.replace("/", "_"))
    with open(local, "w", encoding="utf-8", newline="\n") as fh:
        json.dump(cfg, fh, indent=4)
        fh.write("\n")
    upload(local, "%s/config.json" % modroot)
    print("  done: %s on %s" % (fighter, ", ".join("c%02d" % s for s in slots)))


if __name__ == "__main__":
    main()
