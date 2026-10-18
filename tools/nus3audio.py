#!/usr/bin/env python3
"""Reads and rewrites `.nus3audio` sound banks -- list, extract, and replace entries by index.

    python tools/nus3audio.py list    <bank.nus3audio>
    python tools/nus3audio.py extract <bank.nus3audio> <outdir>
    python tools/nus3audio.py replace <bank.nus3audio> <out.nus3audio> <index>=<file> [...]

WHY BY INDEX, AND NEVER BY ADDING ENTRIES. A bank is two files: this one holds the audio, and the
`.nus3bank` beside it holds the playback metadata the game looks entries up through. An entry
added here has no row there, so nothing can ever play it. Replacing the audio at an existing index
keeps every label and every metadata row, which is the shape the Gogeta moveset ships (its
`se_dolly_c103` borrows Min Min's own c00 label list through `share-to-added`), and ARCropolis
patches the `.nus3bank` sizes from the audio it is given.

THE LAYOUT, read off `se_pikachu.nus3audio` pulled from console #2 (13.0.4), little-endian:

    "NUS3" u32 size
    "AUDIINDX" u32 4, u32 count
    "TNID" u32 len, u32 id[count]                 track ids
    "NMOF" u32 len, u32 name_offset[count]        absolute offsets into TNNM
    "ADOF" u32 len, (u32 offset, u32 size)[count] absolute offsets into PACK
    "TNNM" u32 len, NUL-terminated names
    "JUNK" u32 len, padding so PACK's data is aligned
    "PACK" u32 len, the audio blobs (each an .idsp/.lopus file, 16-byte aligned)

Writing rebuilds every offset from scratch rather than patching them in place, so a replacement of
any size is safe, and `list` on the output must match `list` on the input except for the sizes
that were meant to change -- `replace` checks that before it writes.
"""
import os
import struct
import sys

ALIGN = 0x10


def parse(data):
    if data[:4] != b"NUS3":
        sys.exit("  not a NUS3 file")
    i = 8
    if data[i:i + 8] != b"AUDIINDX":
        sys.exit("  no AUDIINDX")
    count = struct.unpack_from("<I", data, i + 12)[0]
    i += 16
    sec = {}
    while i < len(data) - 8:
        tag = data[i:i + 4].decode("ascii", "replace")
        size = struct.unpack_from("<I", data, i + 4)[0]
        sec[tag] = (i + 8, size)
        if tag == "PACK":
            break
        i += 8 + size
    for need in ("TNID", "NMOF", "ADOF", "TNNM", "PACK"):
        if need not in sec:
            sys.exit("  missing %s section" % need)
    ids = struct.unpack_from("<%dI" % count, data, sec["TNID"][0])
    nmof = struct.unpack_from("<%dI" % count, data, sec["NMOF"][0])
    adof = struct.unpack_from("<%dI" % (count * 2), data, sec["ADOF"][0])
    entries = []
    for k in range(count):
        end = data.index(b"\0", nmof[k])
        name = data[nmof[k]:end].decode("utf-8", "replace")
        off, size = adof[2 * k], adof[2 * k + 1]
        entries.append({"id": ids[k], "name": name, "data": data[off:off + size], "key": (off, size)})
    return entries


def pad(b, align=ALIGN):
    return b + b"\0" * ((-len(b)) % align)


def build(entries):
    count = len(entries)
    names = b""
    name_rel = []
    for e in entries:
        name_rel.append(len(names))
        names += e["name"].encode("utf-8") + b"\0"
    # Header up to and including TNNM, then JUNK sized so PACK's DATA starts aligned.
    head_len = 8 + 16 + (8 + 4 * count) + (8 + 4 * count) + (8 + 8 * count)
    tnnm_data = head_len + 8
    after_tnnm = tnnm_data + len(names)
    junk_len = (-(after_tnnm + 8 + 8)) % ALIGN
    pack_data = after_tnnm + 8 + junk_len + 8
    # ENTRIES SHARE BLOBS. `se_pikachu` has 61 entries over 45 blobs, one of them used nine times,
    # so each blob is written once and every entry that pointed at it points at the copy. A
    # replaced entry gets a key of its own, so replacing one user of a shared blob leaves the
    # others on the original. Blobs keep their original order, which makes an untouched bank
    # rebuild byte-identical -- the test that this writer is right.
    order = sorted({e["key"] for e in entries}, key=lambda k: (k[0], k[1:] != (k[1],)))
    blob_of = {e["key"]: e["data"] for e in entries}
    pack = b""
    where = {}
    for key in order:
        pack = pad(pack)
        where[key] = pack_data + len(pack)
        pack += blob_of[key]
    adof = [(where[e["key"]], len(e["data"])) for e in entries]
    out = bytearray()
    out += b"AUDIINDX" + struct.pack("<II", 4, count)
    out += b"TNID" + struct.pack("<I", 4 * count) + struct.pack("<%dI" % count, *[e["id"] for e in entries])
    out += b"NMOF" + struct.pack("<I", 4 * count) + struct.pack(
        "<%dI" % count, *[tnnm_data + r for r in name_rel])
    flat = [v for pair in adof for v in pair]
    out += b"ADOF" + struct.pack("<I", 8 * count) + struct.pack("<%dI" % (2 * count), *flat)
    out += b"TNNM" + struct.pack("<I", len(names)) + names
    out += b"JUNK" + struct.pack("<I", junk_len) + b"\0" * junk_len
    out += b"PACK" + struct.pack("<I", len(pack)) + pack
    return b"NUS3" + struct.pack("<I", len(out)) + bytes(out)


def kind(blob):
    return blob[:4].decode("ascii", "replace") if blob[:4].isascii() else blob[:4].hex()


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    op, path = sys.argv[1], sys.argv[2]
    entries = parse(open(path, "rb").read())
    if op == "list":
        for k, e in enumerate(entries):
            print("%3d  id=%-4d %-6s %8d  %s" % (k, e["id"], kind(e["data"]), len(e["data"]), e["name"]))
    elif op == "extract":
        out = sys.argv[3]
        os.makedirs(out, exist_ok=True)
        for k, e in enumerate(entries):
            ext = {"IDSP": "idsp", "OPUS": "lopus"}.get(kind(e["data"]), "bin")
            with open(os.path.join(out, "%03d_%s.%s" % (k, e["name"], ext)), "wb") as fh:
                fh.write(e["data"])
        print("  extracted %d entries to %s" % (len(entries), out))
    elif op == "replace":
        dst = sys.argv[3]
        changed = set()
        for spec in sys.argv[4:]:
            idx, f = spec.split("=", 1)
            idx = int(idx)
            if not 0 <= idx < len(entries):
                sys.exit("  index %d out of range (bank has %d)" % (idx, len(entries)))
            orig = entries[idx]["key"]
            entries[idx] = dict(entries[idx], data=open(f, "rb").read(), key=(orig[0], orig[1], "new", idx))
            changed.add(idx)
        blob = build(entries)
        back = parse(blob)
        for k, (a, b) in enumerate(zip(entries, back)):
            if a["id"] != b["id"] or a["name"] != b["name"] or a["data"] != b["data"]:
                sys.exit("  round trip failed at entry %d; nothing written" % k)
        with open(dst, "wb") as fh:
            fh.write(blob)
        print("  wrote %s: %d entries, %d replaced" % (dst, len(entries), len(changed)))
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
