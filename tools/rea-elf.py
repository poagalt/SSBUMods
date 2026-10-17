#!/usr/bin/env python3
"""Wrap one of the game's modules as an AArch64 ELF that REA (and stock Ghidra) open directly.

    python tools/rea-elf.py main                 # -> work/rea/main.elf
    python tools/rea-elf.py nro gaogaen          # -> work/rea/lua2cpp_gaogaen.elf
    python tools/rea-elf.py all                  # main plus every NRO in vendor/nro and work/arc-nro

WHY THIS EXISTS. REA's native analysis goes through Ghidra, and the released rea-agents (6.4.0)
admits a native target only when Ghidra can detect its format from the header: ELF, PE, Mach-O.
`main` here is a headerless memory dump (vendor/main/README-rat.md) and the fighters are NROs,
so neither opens. Its raw-binary mode exists only on REA's unreleased main branch, and even
there it maps one blob at one address with no names. An ELF carries everything at once: the
four segments at their real module offsets, the module's own relocations already applied, and
a symbol table, so a REA session has names, strings, data and cross-references from the start.

EVERYTHING COMES FROM THE MODULE'S OWN TABLES, not from this project's derived lists:

  - MOD0 (text[4:8]) gives .dynamic and the bss bounds.
  - .dynsym gives every export, with its type and size.
  - .rela.dyn / .rela.plt are APPLIED here, exactly as rtld would at this script's base, so
    vtables, function tables and GOT slots hold real addresses inside the ELF.
  - Each PLT stub is named after the import its GOT slot is bound to by R_AARCH64_JUMP_SLOT,
    so a call into nnSdk or into `main` from an NRO reads as the callee's name.
  - Every import gets a named slot in a synthetic .extern block, and each GOT entry or vtable
    word bound to it points there, as Ghidra's own loader presents an unresolved import.

The project's symbol files (vendor/main/*-symbols.txt) are used as a CROSS-CHECK and for names
the module tables do not carry; the counts of each are printed and kept in the manifest.

ADDRESSES MATCH THE REST OF THE PROJECT: `main` at 0x0 (module-relative, as in symbols-merged.txt
and decomp-many.sh), NROs at 0x7100000000 (as in *-nro-symbols.txt and work/decomp). The ELF is
ET_EXEC, because Ghidra rebases an ET_DYN that links at 0 to 0x100000.

MAIN'S .data IS FROM THE RUNNING GAME. rat-main-data.bin was copied out of a live process
(base in rat-memserve.txt), so every pointer in it was relocated for that run. Relocation
targets are rewritten from the module's own .rela, which is exact. The remaining words that
fall inside that run's module image are pointers the game stored at runtime (static
constructors writing vtables, function tables built during init); those are rebased too and
counted separately as "runtime pointers". .rodata was measured to hold no absolute pointers
(tools/emu-module.py docstring), so it is copied verbatim. .bss is left NOBITS: the dump of it
is runtime state, not anything the loader put there.

FUNCTION SEEDS. Ghidra creates a function at every STT_FUNC symbol, so a seed in the middle of a
real function splits it (handbook/data-index.md: 667 spurious boundaries from `br` labels). Only
link-time facts seed: exports, PLT stubs, `bl` targets (work/acquire/main-entrypoints.txt or the
module's work/decomp/<m>.todo.all), and targets of the module's own RELATIVE relocations into
.text, which is where vtables and function tables point. Pointers that exist only at runtime
are NOT seeded; auto-analysis reaches what they reach by flow.
"""

import argparse
import hashlib
import json
import os
import struct
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
VMAIN = os.path.join(ROOT, "vendor", "main")
NRODIR = os.path.join(ROOT, "vendor", "nro", "prebuilt", "nro", "release")
# The 76 fighter NROs vendor/nro does not hold, pulled later from work/missing-nro-paths.txt.
# Together the two hold 96 of the arc's 98; lua2cpp_item and lua2cpp_trail are not local.
NRODIRS = (NRODIR, os.path.join(ROOT, "work", "arc-nro", "prebuilt", "nro", "release"),
           # where a full local extraction of data.arc puts them (tools/switch-session.sh post-dump)
           os.path.join(ROOT, "extracted", "prebuilt", "nro", "release"))


def nro_path(mod):
    for d in NRODIRS:
        p = os.path.join(d, f"lua2cpp_{mod}.nro")
        if os.path.exists(p):
            return p
    return None


def all_nro_modules():
    mods = set()
    for d in NRODIRS:
        if os.path.isdir(d):
            mods |= {f[len("lua2cpp_"):-len(".nro")] for f in os.listdir(d)
                     if f.startswith("lua2cpp_") and f.endswith(".nro")}
    return sorted(mods)
OUT = os.path.join(ROOT, "work", "rea")
DISASM = os.path.join(ROOT, "tools", "disasm", "target", "release", "disasm.exe")
NRO_BASE = 0x7100000000

DT = dict(NULL=0, NEEDED=1, PLTRELSZ=2, PLTGOT=3, HASH=4, STRTAB=5, SYMTAB=6, RELA=7,
          RELASZ=8, RELAENT=9, STRSZ=10, SYMENT=11, INIT=12, FINI=13, PLTREL=20, JMPREL=23,
          INIT_ARRAY=25, FINI_ARRAY=26, RELRSZ=35, RELR=36, GNU_HASH=0x6FFFFEF5)
R_ABS64, R_GLOB_DAT, R_JUMP_SLOT, R_RELATIVE = 257, 1025, 1026, 1027


def log(msg):
    print(msg, flush=True)


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def u32(b, o):
    return struct.unpack_from("<I", b, o)[0]


def u64(b, o):
    return struct.unpack_from("<Q", b, o)[0]


def read_regions(path):
    regions = {}
    for line in open(path):
        parts = line.split()
        if len(parts) >= 2 and not line.startswith("#") and parts[0] in ("text", "rodata", "data", "bss"):
            regions[parts[0]] = [int(p, 16) for p in parts[1:3]]
    return regions


def read_addr_list(path):
    """`0x…` or bare hex per line, optional name after it. Returns {addr: name or None}."""
    out = {}
    if not os.path.exists(path):
        return out
    for line in open(path):
        parts = line.split()
        if not parts or parts[0].startswith("#"):
            continue
        a = int(parts[0], 16)
        out[a] = parts[1] if len(parts) > 1 else None
    return out


def read_symbol_file(path):
    """`addr size name [def|plt]` rows: the format of vendor/main/*-symbols.txt."""
    rows = []
    if not os.path.exists(path):
        return rows
    for line in open(path):
        parts = line.split()
        if len(parts) < 3 or parts[0].startswith("#"):
            continue
        rows.append((int(parts[0], 16), int(parts[1], 16), parts[2], parts[3] if len(parts) > 3 else None))
    return rows


class Module:
    """A module image laid out at module-relative offsets, plus where its segments are."""

    def __init__(self, name, image, segs, bss, base, inputs):
        self.name, self.image, self.segs, self.bss, self.base = name, image, segs, bss, base
        self.inputs = inputs
        self.notes = {}


def load_main():
    text_p = os.path.join(VMAIN, "rat-main-text.bin")
    ro_p = os.path.join(VMAIN, "rat-main-rodata.bin")
    data_p = os.path.join(VMAIN, "rat-main-data.bin")
    mem = read_regions(os.path.join(VMAIN, "rat-memserve.txt"))
    exe = read_regions(os.path.join(VMAIN, "rat-exedump.txt"))
    text, ro, data = (open(p, "rb").read() for p in (text_p, ro_p, data_p))

    run_base = mem["text"][0]
    ro_off = mem["rodata"][0] - run_base
    data_off = mem["data"][0] - run_base
    # The two runs must agree on the layout, or the rodata from one cannot sit beside the data
    # from the other.
    if exe["rodata"][0] - exe["text"][0] != ro_off or exe["data"][0] - exe["text"][0] != data_off:
        sys.exit("rat-exedump.txt and rat-memserve.txt disagree on main's segment offsets")
    if (len(text), len(ro), len(data)) != (mem["text"][1], mem["rodata"][1], mem["data"][1]):
        sys.exit("dump sizes do not match rat-memserve.txt")
    if len(text) != ro_off or ro_off + len(ro) != data_off:
        sys.exit("main's segments are not contiguous; this script assumes they are")

    image = bytearray(text + ro + data)
    m = Module("main", image,
               {"text": (0, len(text)), "rodata": (ro_off, len(ro)), "data": (data_off, len(data))},
               None, 0, {p: sha256_file(p) for p in (text_p, ro_p, data_p)})
    m.run_base = run_base
    return m


def load_nro(path):
    blob = open(path, "rb").read()
    if blob[0x10:0x14] != b"NRO0":
        sys.exit(f"{path}: no NRO0 magic")
    t_off, t_sz, r_off, r_sz, d_off, d_sz = struct.unpack_from("<6I", blob, 0x20)
    if t_off != 0 or t_sz != r_off or r_off + r_sz != d_off:
        sys.exit(f"{path}: NRO segments are not contiguous from 0")
    image = bytearray(blob[: d_off + d_sz])
    m = Module(os.path.splitext(os.path.basename(path))[0], image,
               {"text": (0, t_sz), "rodata": (r_off, r_sz), "data": (d_off, d_sz)},
               None, NRO_BASE, {path: sha256_file(path)})
    m.run_base = None
    m.inputs_path = path
    return m


def lz4_block(src, out_size):
    """LZ4 block format, as NSO segments use it. Checked against each segment's SHA-256."""
    dst, i, n = bytearray(), 0, len(src)
    while i < n:
        token = src[i]; i += 1
        lit = token >> 4
        if lit == 15:
            while True:
                b = src[i]; i += 1; lit += b
                if b != 255:
                    break
        dst += src[i:i + lit]; i += lit
        if i >= n:
            break
        off = src[i] | (src[i + 1] << 8); i += 2
        ml = token & 15
        if ml == 15:
            while True:
                b = src[i]; i += 1; ml += b
                if b != 255:
                    break
        ml += 4
        start = len(dst) - off
        if off >= ml:
            dst += dst[start:start + ml]
        else:  # overlapping match: the copy reads bytes it is writing
            for k in range(ml):
                dst.append(dst[start + k])
    if len(dst) != out_size:
        raise ValueError(f"LZ4 produced {len(dst)} bytes, header says {out_size}")
    return dst


def read_pfs0(path, member):
    """One file out of a PFS0 container (an ExeFS, an .nsp)."""
    blob = open(path, "rb").read()
    if blob[:4] != b"PFS0":
        sys.exit(f"{path}: not a PFS0 container")
    count, strsz = struct.unpack_from("<II", blob, 4)
    strtab = 0x10 + 0x18 * count
    data0 = strtab + strsz
    for k in range(count):
        off, size, name_off = struct.unpack_from("<QQI", blob, 0x10 + 0x18 * k)
        name = blob[strtab + name_off:blob.index(b"\0", strtab + name_off)].decode()
        if name == member:
            return blob[data0 + off:data0 + off + size]
    sys.exit(f"{path}: no member {member!r}")


def load_nso(path, base, name, member="main"):
    """A real NSO, from an ExeFS dump (nxdumptool), as a module image at `base`.

    The memory dumps load_main() reads are what a running game holds; an NSO is what the loader
    started from, so its .data is the initial image and needs no runtime-pointer rebasing.
    Each segment's SHA-256 in the header is checked after decompression.
    """
    blob = open(path, "rb").read()
    if blob[:4] == b"PFS0":
        blob = read_pfs0(path, member)
    if blob[:4] != b"NSO0":
        sys.exit(f"{path}: not an NSO")
    flags = u32(blob, 0xC)
    segs, image_end = [], 0
    for k, seg in enumerate(("text", "rodata", "data")):
        f_off, m_off, size = struct.unpack_from("<III", blob, 0x10 + 0x10 * k)
        csize = u32(blob, 0x60 + 4 * k)
        raw = blob[f_off:f_off + (csize if flags & (1 << k) else size)]
        data = lz4_block(raw, size) if flags & (1 << k) else bytearray(raw)
        want = blob[0xA0 + 0x20 * k:0xC0 + 0x20 * k]
        if flags & (8 << k) or any(want):
            got = hashlib.sha256(data).digest()
            if got != want:
                sys.exit(f"{path}: {seg} SHA-256 mismatch after decompression")
        segs.append((seg, m_off, data))
        image_end = max(image_end, m_off + size)
    image = bytearray(image_end)
    for seg, m_off, data in segs:
        image[m_off:m_off + len(data)] = data
    # Segments are page-aligned in memory; the gap after each is part of it for our purposes.
    t, r, d = segs
    m = Module(name, image, {"text": (t[1], r[1] - t[1]), "rodata": (r[1], d[1] - r[1]),
                             "data": (d[1], len(d[2]))}, None, base, {path: sha256_file(path)})
    m.run_base = None
    m.inputs_path = path
    m.notes["nso_build_id"] = blob[0x40:0x60].hex()
    m.notes["nso_flags"] = hex(flags)
    return m


def parse_mod0(m):
    img = m.image
    off = u32(img, 4)
    if img[off:off + 4] != b"MOD0":
        sys.exit(f"{m.name}: no MOD0 at 0x{off:x}")
    dyn, bss_s, bss_e = (off + struct.unpack_from("<i", img, off + 4 + 4 * i)[0] for i in range(3))
    m.dyn_off = dyn
    m.bss = (bss_s, bss_e - bss_s)
    d_off, d_sz = m.segs["data"]
    if bss_s < d_off + d_sz - 0x1000 or bss_s > d_off + d_sz + 0x1000:
        sys.exit(f"{m.name}: MOD0 bss start 0x{bss_s:x} is not at the end of .data (0x{d_off + d_sz:x})")
    # An NRO's .bss begins inside the last page of .data, and Ghidra refuses overlapping memory
    # blocks, so .data ends where MOD0 says .bss begins. The bytes cut off must be zero padding.
    if bss_s < d_off + d_sz:
        if any(img[bss_s:d_off + d_sz]):
            sys.exit(f"{m.name}: bytes between MOD0 bss start and the end of .data are not zero")
        m.segs["data"] = (d_off, bss_s - d_off)


def parse_dynamic(m):
    img, dyn = m.image, {}
    o = m.dyn_off
    while True:
        tag, val = struct.unpack_from("<qQ", img, o)
        o += 16
        if tag == DT["NULL"]:
            break
        dyn.setdefault(tag, []).append(val)
    m.dyn = {k: v[0] for k, v in dyn.items()}
    for need in ("SYMTAB", "STRTAB"):
        if DT[need] not in m.dyn:
            sys.exit(f"{m.name}: .dynamic has no DT_{need}")


def dynsym_count(m):
    d, img = m.dyn, m.image
    if DT["HASH"] in d:
        return u32(img, d[DT["HASH"]] + 4)
    if DT["GNU_HASH"] in d:
        h = d[DT["GNU_HASH"]]
        nbuckets, symoffset, bloom = u32(img, h), u32(img, h + 4), u32(img, h + 8)
        buckets = h + 16 + bloom * 8
        chains = buckets + nbuckets * 4
        last = max(u32(img, buckets + 4 * i) for i in range(nbuckets))
        if last < symoffset:
            return symoffset
        while not (u32(img, chains + 4 * (last - symoffset)) & 1):
            last += 1
        return last + 1
    return (d[DT["STRTAB"]] - d[DT["SYMTAB"]]) // 24


def parse_dynsym(m):
    img, d = m.image, m.dyn
    symtab, strtab = d[DT["SYMTAB"]], d[DT["STRTAB"]]

    def cstr(o):
        e = img.index(b"\0", strtab + o)
        return img[strtab + o:e].decode("utf-8", "replace")

    m.dynsym = []
    for i in range(dynsym_count(m)):
        name_o, info, other, shndx, value, size = struct.unpack_from("<IBBHQQ", img, symtab + 24 * i)
        m.dynsym.append(dict(name=cstr(name_o) if name_o else "", bind=info >> 4, type=info & 0xF,
                             shndx=shndx, value=value, size=size))


def rela_entries(m, start_tag, size_tag):
    d = m.dyn
    if start_tag not in d:
        return []
    start, size = d[start_tag], d.get(size_tag, 0)
    return [struct.unpack_from("<QQq", m.image, start + o) for o in range(0, size, 24)]


def relr_offsets(m):
    d = m.dyn
    if DT["RELR"] not in d:
        return []
    start, size, out, where = d[DT["RELR"]], d.get(DT["RELRSZ"], 0), [], 0
    for o in range(start, start + size, 8):
        e = u64(m.image, o)
        if not e & 1:
            out.append(e)
            where = e + 8
        else:
            for bit in range(63):
                if (e >> (bit + 1)) & 1:
                    out.append(where + 8 * bit)
            where += 63 * 8
    return out


def apply_relocations(m):
    """Write what rtld would write at m.base.

    An import lives in another module, so it gets an 8-byte slot in a synthetic .extern block
    after .bss, named after the import, and every reference to it points there. That is how
    Ghidra's own ELF loader presents an unresolved import (its EXTERNAL block), so a vtable
    slot or GOT entry reads as `PTR_<import>` instead of a null.
    """
    img, base = m.image, m.base
    counts, imports, unknown = {}, {}, {}
    m.reloc_targets, m.relative_text, m.own_slots = set(), set(), set()
    t_off, t_sz = m.segs["text"]
    m.extern_off = (m.bss[0] + m.bss[1] + 0xFFF) & ~0xFFF
    m.externs = {}  # name -> slot offset

    def extern(name):
        if name not in m.externs:
            m.externs[name] = m.extern_off + 8 * len(m.externs)
        return m.externs[name]

    def put(off, val):
        struct.pack_into("<Q", img, off, val & 0xFFFFFFFFFFFFFFFF)
        m.reloc_targets.add(off)

    rels = rela_entries(m, DT["RELA"], DT["RELASZ"]) + rela_entries(m, DT["JMPREL"], DT["PLTRELSZ"])
    for off, info, addend in rels:
        rtype, sym = info & 0xFFFFFFFF, info >> 32
        counts[rtype] = counts.get(rtype, 0) + 1
        if rtype == R_RELATIVE:
            put(off, base + addend)
            if t_off <= addend < t_off + t_sz and addend % 4 == 0:
                m.relative_text.add(addend)
        elif rtype in (R_ABS64, R_GLOB_DAT, R_JUMP_SLOT):
            s = m.dynsym[sym]
            if s["shndx"] != 0:
                put(off, base + s["value"] + addend)
                if rtype == R_JUMP_SLOT:
                    m.own_slots.add(off)
            else:
                put(off, base + extern(s["name"]) + addend)
                imports[off] = (s["name"], rtype, s["type"])
        else:
            unknown[rtype] = unknown.get(rtype, 0) + 1
    for off in relr_offsets(m):
        put(off, base + u64(img, off))
        counts["RELR"] = counts.get("RELR", 0) + 1
    m.imports = imports
    m.extern_size = max(8 * len(m.externs), 8)
    m.notes["relocations"] = {str(k): v for k, v in sorted(counts.items(), key=lambda kv: str(kv[0]))}
    if unknown:
        m.notes["relocations_not_applied"] = {str(k): v for k, v in unknown.items()}
        log(f"  WARNING: relocation types not applied: {unknown}")


def rebase_runtime_pointers(m):
    """main only: words in .data that point into the dump run's module image, but that no
    relocation wrote, were stored by the game at runtime. Rebase them onto m.base."""
    if m.run_base is None:
        return
    img = m.image
    d_off, d_sz = m.segs["data"]
    end = m.bss[0] + m.bss[1]
    lo, hi = m.run_base, m.run_base + end
    n = 0
    for off in range(d_off, d_off + d_sz, 8):
        if off in m.reloc_targets:
            continue
        v = u64(img, off)
        if lo <= v < hi:
            struct.pack_into("<Q", img, off, m.base + v - lo)
            n += 1
    m.notes["runtime_pointers_rebased"] = n


def find_plt_stubs(m):
    """Standard AArch64 PLT entries: adrp x16 / ldr x17,[x16,#lo] / add x16,x16,#lo / br x17."""
    img = m.image
    t_off, t_sz = m.segs["text"]
    stubs, br = {}, b"\x20\x02\x1f\xd6"
    # A stub whose slot the module binds to ITS OWN export (an interposable symbol it calls
    # through the PLT) stays unnamed: Ghidra resolves it as a thunk to the definition, and giving
    # it the definition's name would make every such name ambiguous to REA's name lookup.
    m.own_stubs = 0
    i = img.find(br, t_off + 12, t_off + t_sz)
    while i != -1:
        if i % 4 == 0:
            p = i - 12
            adrp, ldr, add = u32(img, p), u32(img, p + 4), u32(img, p + 8)
            if (adrp & 0x9F00001F) == 0x90000010 and (ldr & 0xFFC003FF) == 0xF9400211 \
                    and (add & 0xFFC003FF) == 0x91000210:
                imm = ((adrp >> 29) & 3) | (((adrp >> 5) & 0x7FFFF) << 2)
                if imm & (1 << 20):
                    imm -= 1 << 21
                got = (p & ~0xFFF) + (imm << 12) + ((ldr >> 10) & 0xFFF) * 8
                if got in m.imports:
                    stubs[p] = m.imports[got][0]
                elif got in m.own_slots:
                    m.own_stubs += 1
        i = img.find(br, i + 4, t_off + t_sz)
    return stubs


def seg_of(m, off):
    for idx, (name, (o, s)) in enumerate([(k, m.segs[k]) for k in ("text", "rodata", "data")]):
        if o <= off < o + s:
            return idx + 1, name
    b_o, b_s = m.bss
    if b_o <= off < b_o + b_s:
        return 4, "bss"
    if m.extern_off <= off < m.extern_off + m.extern_size:
        return 5, "extern"
    return None, None


def build_symbols(m, extra_names, seeds):
    """Returns [(name, value, size, type, bind, shndx)] in module-relative offsets."""
    syms, by_addr = [], {}

    def add(name, off, size, stype, bind):
        shndx, _ = seg_of(m, off)
        if shndx is None or not name:
            return False
        if name in by_addr.get(off, ()):
            return False
        by_addr.setdefault(off, set()).add(name)
        syms.append((name, off, size, stype, bind, shndx))
        return True

    n_exports = 0
    for s in m.dynsym:
        if s["shndx"] != 0 and s["name"] and s["type"] in (0, 1, 2):
            n_exports += add(s["name"], s["value"], s["size"], s["type"], s["bind"] or 1)
    stubs = find_plt_stubs(m)
    for off, name in stubs.items():
        add(name, off, 16, 2, 1)
    # OBJECT, never FUNC: there are no bytes there, and a FUNC would have Ghidra create a
    # function in empty memory. A procedure lookup by name therefore still finds only the stub.
    for name, off in m.externs.items():
        add(name, off, 8, 1, 1)

    # A project-file name the module's own tables already put somewhere else is not repeated:
    # REA's lookup by name refuses an ambiguous one.
    table_names = {n for names in by_addr.values() for n in names}
    extra = extra_dup = 0
    for off, size, name, _kind in extra_names:
        if off - m.base < 0 or name in by_addr.get(off - m.base, ()):
            continue
        if name in table_names:
            extra_dup += 1
            continue
        shndx, seg = seg_of(m, off - m.base)
        stype = 2 if seg == "text" else 1
        extra += add(name, off - m.base, size, stype, 0)

    t_off, t_sz = m.segs["text"]
    seeds = set(seeds) | {m.base + a for a in m.relative_text}
    seeded = 0
    for off in sorted(seeds):
        rel = off - m.base
        if not (t_off <= rel < t_off + t_sz) or rel in by_addr:
            continue
        seeded += add("sub_%x" % off, rel, 0, 2, 0)

    m.notes.update(dynsym=len(m.dynsym), exports_named=n_exports, imports=len(m.imports),
                   plt_stubs_named=len(stubs), plt_stubs_to_own_exports=m.own_stubs,
                   extern_slots=len(m.externs),
                   extra_names_from_project=extra, extra_names_skipped_as_duplicates=extra_dup, function_seeds=seeded,
                   relative_relocs_into_text=len(m.relative_text))
    m.stubs = stubs
    return syms


def write_elf(m, syms, path):
    base, img = m.base, m.image
    PAGE = 0x1000
    bss_end = m.bss[0] + m.bss[1]
    img_file_off = PAGE

    # String tables.
    strtab, str_idx = bytearray(b"\0"), {}

    def sname(s):
        if s not in str_idx:
            str_idx[s] = len(strtab)
            strtab.extend(s.encode() + b"\0")
        return str_idx[s]

    syms = sorted(syms, key=lambda s: (s[4] != 0, s[1]))
    first_global = next((i + 1 for i, s in enumerate(syms) if s[4] != 0), len(syms) + 1)
    symtab = bytearray(24)
    for name, off, size, stype, bind, shndx in syms:
        symtab += struct.pack("<IBBHQQ", sname(name), (bind << 4) | stype, 0, shndx, base + off, size)

    shstr = bytearray(b"\0")
    sh_names = {}
    for n in (".text", ".rodata", ".data", ".bss", ".extern", ".symtab", ".strtab", ".shstrtab"):
        sh_names[n] = len(shstr)
        shstr += n.encode() + b"\0"

    symtab_off = img_file_off + len(img)
    symtab_off = (symtab_off + 7) & ~7
    strtab_off = symtab_off + len(symtab)
    shstr_off = strtab_off + len(strtab)
    shdr_off = (shstr_off + len(shstr) + 7) & ~7

    t_o, t_s = m.segs["text"]
    r_o, r_s = m.segs["rodata"]
    d_o, d_s = m.segs["data"]
    phdrs = [
        (1, 5, img_file_off + t_o, base + t_o, t_s, t_s),                   # R X
        (1, 4, img_file_off + r_o, base + r_o, r_s, r_s),                   # R
        (1, 6, img_file_off + d_o, base + d_o, d_s, bss_end - d_o),         # RW, bss in memsz
        # Imports. No file bytes, but the offset must still lie inside the file and agree with the
        # vaddr modulo the page, or Ghidra drops the segment as having an invalid offset.
        (1, 4, img_file_off, base + m.extern_off, 0, m.extern_size),
    ]
    init = m.dyn.get(DT["INIT"])
    entry = base + (init if init is not None else 0)

    shdrs = [
        (0, 0, 0, 0, 0, 0, 0, 0, 0, 0),
        (sh_names[".text"], 1, 6, base + t_o, img_file_off + t_o, t_s, 0, 0, 16, 0),
        (sh_names[".rodata"], 1, 2, base + r_o, img_file_off + r_o, r_s, 0, 0, 16, 0),
        (sh_names[".data"], 1, 3, base + d_o, img_file_off + d_o, d_s, 0, 0, 16, 0),
        (sh_names[".bss"], 8, 3, base + m.bss[0], img_file_off + m.bss[0], m.bss[1], 0, 0, 16, 0),
        (sh_names[".extern"], 8, 2, base + m.extern_off, img_file_off + m.extern_off, m.extern_size,
         0, 0, 8, 0),
        (sh_names[".symtab"], 2, 0, 0, symtab_off, len(symtab), 7, first_global, 8, 24),
        (sh_names[".strtab"], 3, 0, 0, strtab_off, len(strtab), 0, 0, 1, 0),
        (sh_names[".shstrtab"], 3, 0, 0, shstr_off, len(shstr), 0, 0, 1, 0),
    ]
    ehdr = b"\x7fELF" + bytes([2, 1, 1, 0]) + bytes(8)
    ehdr += struct.pack("<HHIQQQIHHHHHH", 2, 183, 1, entry, 64, shdr_off, 0, 64, 56, len(phdrs),
                        64, len(shdrs), len(shdrs) - 1)

    tmp = path + ".tmp"
    with open(tmp, "wb") as f:
        f.write(ehdr)
        for p in phdrs:
            f.write(struct.pack("<IIQQQQQQ", p[0], p[1], p[2], p[3], p[3], p[4], p[5], PAGE))
        f.write(bytes(img_file_off - f.tell()))
        f.write(img)
        f.write(bytes(symtab_off - f.tell()))
        f.write(symtab)
        f.write(strtab)
        f.write(shstr)
        f.write(bytes(shdr_off - f.tell()))
        for s in shdrs:
            f.write(struct.pack("<IIQQQQIIQQ", *s))
    os.replace(tmp, path)
    return len(syms)


def cross_check(m, symfile_rows):
    """How many of the project's own rows does the module's table reproduce?"""
    if not symfile_rows:
        return
    ours = {}
    for s in m.dynsym:
        if s["shndx"] != 0 and s["name"]:
            ours.setdefault(m.base + s["value"], set()).add(s["name"])
    for off, name in m.stubs.items():
        ours.setdefault(m.base + off, set()).add(name)
    hit = sum(1 for a, _, n, _ in symfile_rows if n in ours.get(a, ()))
    m.notes["project_symbol_rows"] = len(symfile_rows)
    m.notes["project_symbol_rows_reproduced"] = hit
    log(f"  cross-check: {hit} of {len(symfile_rows)} project symbol rows reproduced from the module's own tables")


def status_names(m, mod):
    """Name the status functions an NRO registers: `<STATUS_KIND>__<FUNC>`, for example
    FIGHTER_GAOGAEN_STATUS_KIND_SPECIAL_S_THROW__STATUS_MAIN.

    The map is `disasm --statusmap` run from the module's create_agent_fighter_status_script,
    which reads the `sv_set_status_func` calls the module itself makes. Cached as
    work/rea/<mod>-statusmap.txt; vendor/main/<mod>-statusmap.txt is used when it exists.
    """
    cached = [os.path.join(VMAIN, f"{mod}-statusmap.txt"), os.path.join(OUT, f"{mod}-statusmap.txt")]
    path = next((c for c in cached if os.path.exists(c)), None)
    if path is None:
        agent = next((s for s in m.dynsym if s["shndx"] and
                      f"create_agent_fighter_status_script_{mod}E" in s["name"]), None)
        if agent is None:
            m.notes["status_map"] = "module registers no fighter status script"
            return []
        if not os.path.exists(DISASM):
            m.notes["status_map"] = "skipped: tools/disasm is not built"
            return []
        r = subprocess.run([DISASM, m.inputs_path, "--statusmap", "0x%x" % (m.base + agent["value"])],
                           capture_output=True, text=True)
        if r.returncode != 0:
            sys.exit(f"{mod}: disasm --statusmap failed: {r.stderr.strip()[-300:]}")
        path = cached[1]
        os.makedirs(OUT, exist_ok=True)
        with open(path, "w", newline="\n") as f:
            f.write(r.stdout)
    rows = []
    for line in open(path):
        parts = line.split()
        if len(parts) >= 3 and parts[0].startswith("0x"):
            func = parts[2].replace("LUA_SCRIPT_STATUS_FUNC_", "")
            rows.append((int(parts[0], 16), 0, f"{parts[1]}__{func}", None))
    m.notes["status_map"] = os.path.relpath(path, ROOT).replace("\\", "/")
    m.notes["status_functions_named"] = len(rows)
    return rows


def build(m, extra_names, seeds, out_path, symfile_rows, late_names=None):
    log(f"{m.name}: {len(m.image):,} bytes of image at base 0x{m.base:x}")
    parse_mod0(m)
    parse_dynamic(m)
    parse_dynsym(m)
    if late_names:
        extra_names = list(extra_names) + late_names(m)
    apply_relocations(m)
    rebase_runtime_pointers(m)
    syms = build_symbols(m, extra_names, seeds)
    cross_check(m, symfile_rows)
    os.makedirs(OUT, exist_ok=True)
    n = write_elf(m, syms, out_path)
    manifest = dict(module=m.name, elf=os.path.relpath(out_path, ROOT).replace("\\", "/"),
                    elf_sha256=sha256_file(out_path), base="0x%x" % m.base,
                    segments={k: ["0x%x" % (m.base + o), "0x%x" % s] for k, (o, s) in m.segs.items()},
                    bss=["0x%x" % (m.base + m.bss[0]), "0x%x" % m.bss[1]],
                    symbols_written=n, counts=m.notes,
                    inputs={os.path.relpath(p, ROOT).replace("\\", "/"): h for p, h in m.inputs.items()},
                    generator="tools/rea-elf.py")
    with open(out_path + ".json", "w") as f:
        json.dump(manifest, f, indent=1)
    log(f"  wrote {manifest['elf']}: {n:,} symbols, " + ", ".join(f"{k}={v}" for k, v in m.notes.items()
                                                              if not isinstance(v, dict)))
    return manifest


def build_main():
    m = load_main()
    merged = os.path.join(VMAIN, "symbols-merged.txt")
    m.inputs[merged] = sha256_file(merged)
    extra = read_symbol_file(merged)
    seeds = set(read_addr_list(os.path.join(ROOT, "work", "acquire", "main-entrypoints.txt")))
    exports = read_symbol_file(os.path.join(VMAIN, "rat-main-symbols.txt"))
    return build(m, extra, seeds, os.path.join(OUT, "main.elf"), exports)


def build_nro(mod):
    path = nro_path(mod)
    if path is None:
        sys.exit(f"no lua2cpp_{mod}.nro in vendor/nro or work/arc-nro; pull it with tools/pull-nro.sh")
    m = load_nro(path)
    rows = read_symbol_file(os.path.join(VMAIN, f"{mod}-nro-symbols.txt"))
    seeds = set(read_addr_list(os.path.join(ROOT, "work", "decomp", f"{mod}.todo.all")))
    return build(m, rows, seeds, os.path.join(OUT, f"lua2cpp_{mod}.elf"), rows,
                 late_names=lambda mm: status_names(mm, mod))


def build_nso(path, base, name, member):
    m = load_nso(path, base, name, member)
    return build(m, [], set(), os.path.join(OUT, f"{name}.elf"), [])


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("main")
    p = sub.add_parser("nro")
    p.add_argument("module", nargs="+", help="fighter module, e.g. gaogaen or common")
    p = sub.add_parser("nso", help="a real NSO (or a PFS0/ExeFS holding one), e.g. from nxdumptool")
    p.add_argument("path")
    p.add_argument("--name", help="output name (default: the file's name)")
    p.add_argument("--member", default="main", help="member of a PFS0 container (default: main)")
    p.add_argument("--base", default="0x0", help="load address (default 0x0, as main is here)")
    sub.add_parser("all")
    a = ap.parse_args()
    if a.cmd == "main":
        build_main()
    elif a.cmd == "nro":
        for mod in a.module:
            build_nro(mod)
    elif a.cmd == "nso":
        name = a.name or os.path.splitext(os.path.basename(a.path))[0]
        build_nso(a.path, int(a.base, 16), name, a.member)
    else:
        build_main()
        for mod in all_nro_modules():
            build_nro(mod)


if __name__ == "__main__":
    main()
