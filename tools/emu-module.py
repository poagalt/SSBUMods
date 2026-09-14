#!/usr/bin/env python3
"""Recover module information from 13.0.5 `main` by emulating it with Unicorn.

`ultimate-research/module-info-extractor` (vendor/refs/module-info-extractor) is the
prior art: it emulates 2.0.0's module initialisation and reads the registration calls
as they happen, which is how `consts.csv` and `main_200_function_list.txt` exist at all.
Its addresses are 2.0.0 and do not transfer; the METHOD does, and this is it for 13.0.5.

Why Python when every other tool here is bash or Rust: Unicorn's supported binding is
the Python one, and the reference implementation is Python. The `unicorn-engine` crate
wants cmake plus a C toolchain against a C library we would then have to keep building.

WHAT MAKES THIS WORK, AND IT IS NOT OBVIOUS: vendor/main/rat-main-{data,bss}.bin were
copied out of the RUNNING game (tools/pull-nro.sh -> src/telemetry, base in
vendor/main/rat-memserve.txt), so every pointer in them is ALREADY RELOCATED for that
run's base of 0x4dd3c04000. Map the four segments at that same base and the module's
own vtables, function tables and `.init_array` all resolve without a linker. `.rodata`
was measured to hold no absolute pointers at all (1 byte-aligned false positive in
22 MB), so mixing it in from the earlier exedump run is safe.

Subcommands, cheapest first:

  ptrs      Harvest function pointers out of .data/.bss. No emulation. This is the
            boundary list the emulation was wanted for -- 70,143 unique text pointers,
            67,651 of them not reachable as a `bl` target -- and it takes 30 seconds.
  find      Locate 13.0.5's constant-registration call site by code xref to a known
            constant name string, the way 2.0.0's hardcoded addresses were found once.
  consts    Emulate the registering function and record every (name, value) it
            registers, with the hash40 the game will look it up by.
  indirect  The long sweep: emulate each known entry point under a budget and record
            every `blr`/`br` target, which is what a static `bl` scan cannot see.

Every subcommand writes results incrementally and prints progress unbuffered, because
the last long run in this project looked wedged for twelve minutes purely because its
output was buffered behind a pipe (see BACKLOG, and tools/deploy.sh:84).
"""

import argparse
import os
import struct
import sys
import time
import zlib

VENDOR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "vendor", "main")
WORK = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "work", "emu")

# From vendor/main/rat-memserve.txt -- the run that produced data/bss, hence the base
# every pointer inside them is relocated for.
BASE = 0x4DD3C04000
SEGS = [
    # name      file                   module offset  size        prot
    ("text",   "rat-main-text.bin",   0x0000000,     0x39C9000,  "rx"),
    ("rodata", "rat-main-rodata.bin", 0x39C9000,     0x154E000,  "r"),
    ("data",   "rat-main-data.bin",   0x4F17000,     0x0393000,  "rw"),
    ("bss",    "rat-main-bss.bin",    0x52AA000,     0x219E000,  "rw"),
]
TEXT_SZ = 0x39C9000

STACK_BASE = 0x7F0000000000
STACK_SZ = 0x400000
HEAP_BASE = 0x7E0000000000
HEAP_SZ = 0x4000000


def log(msg):
    """Progress has to be visible from outside, so every line flushes."""
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", flush=True)


def seg_bytes(name):
    for n, fn, off, size, _ in SEGS:
        if n == name:
            with open(os.path.join(VENDOR, fn), "rb") as f:
                return off, f.read()
    raise KeyError(name)


def known_entrypoints():
    """The 32,857 `bl`-target entry points tools/disasm --callees already recovered."""
    path = os.path.join(os.path.dirname(VENDOR), "..", "work", "acquire", "main-entrypoints.txt")
    out = set()
    with open(os.path.normpath(path)) as f:
        for ln in f:
            ln = ln.strip()
            if ln.startswith("0x"):
                out.add(int(ln, 16))
    return out


def load_symbols():
    """vendor/main/symbols-merged.txt: `offset size name`, offsets module-relative."""
    syms = {}
    with open(os.path.join(VENDOR, "symbols-merged.txt")) as f:
        for ln in f:
            if ln.startswith("#"):
                continue
            parts = ln.split(None, 2)
            if len(parts) == 3:
                try:
                    syms[int(parts[0], 16)] = parts[2].strip()
                except ValueError:
                    pass
    return syms


def hash40(s):
    """The game's own hash: crc32 of the bytes, with the length in the top byte.

    Same formula tools/hash40.sh uses. The reference implementation emulated the
    binary's hashing function for this; there is no reason to, it is this.
    """
    b = s.encode("latin-1")
    return (zlib.crc32(b) & 0xFFFFFFFF) | ((len(b) & 0xFF) << 32)


# ---------------------------------------------------------------------------
# ptrs -- function pointers out of the live-process data, no emulation needed
# ---------------------------------------------------------------------------

def cmd_ptrs(args):
    os.makedirs(WORK, exist_ok=True)
    known = known_entrypoints()
    log(f"{len(known)} known `bl`-target entry points")
    found = set()
    for name in ("data", "bss"):
        _, blob = seg_bytes(name)
        words = struct.unpack_from("<%dQ" % (len(blob) // 8), blob, 0)
        hits = 0
        for v in words:
            if BASE <= v < BASE + TEXT_SZ:
                off = v - BASE
                if off % 4 == 0:
                    found.add(off)
                    hits += 1
        log(f"{name}: {hits} pointers into .text, {len(found)} unique so far")

    new = found - known
    allp = sorted(found | known)
    with open(os.path.join(WORK, "ptr-entrypoints.txt"), "w") as f:
        for a in sorted(found):
            f.write("0x%010x\n" % a)
    with open(os.path.join(WORK, "new-entrypoints.txt"), "w") as f:
        for a in sorted(new):
            f.write("0x%010x\n" % a)
    with open(os.path.join(WORK, "all-entrypoints.txt"), "w") as f:
        for a in allp:
            f.write("0x%010x\n" % a)
    log(f"unique pointer targets {len(found)}, NEW {len(new)}, union with known {len(allp)}")
    log(f"wrote {WORK}/{{ptr,new,all}}-entrypoints.txt")


# ---------------------------------------------------------------------------
# find -- locate the registration call site by code xref to a known string
# ---------------------------------------------------------------------------

def scan_adrp_add(text, target):
    """Every `adrp Xn,page(target)` + `add Xn,Xn,lo12(target)` pair in .text.

    This is how the reference's hardcoded 2.0.0 addresses were found in the first
    place, and it is the only way to find the 13.0.5 equivalents: the registering
    function is not exported, but the constant NAME it passes is a rodata string,
    and whoever materialises that string's address is the caller.
    """
    page = target & ~0xFFF
    lo12 = target & 0xFFF
    n = len(text) // 4
    words = struct.unpack_from("<%dI" % n, text, 0)
    hits = []
    for i in range(n):
        w = words[i]
        if (w & 0x9F000000) != 0x90000000:      # ADRP
            continue
        immlo = (w >> 29) & 3
        immhi = (w >> 5) & 0x7FFFF
        imm = (immhi << 2) | immlo
        if imm & (1 << 20):
            imm -= 1 << 21
        pc = i * 4
        if ((pc & ~0xFFF) + (imm << 12)) != page:
            continue
        rd = w & 0x1F
        for j in range(i + 1, min(i + 9, n)):   # the add is usually adjacent
            v = words[j]
            if (v & 0xFF800000) == 0x91000000 and ((v >> 10) & 0xFFF) == lo12 \
                    and ((v >> 5) & 0x1F) == rd:
                hits.append((pc, j * 4, v & 0x1F))
                break
    return hits


def find_string(needle, seg="rodata"):
    off, blob = seg_bytes(seg)
    at = blob.find(needle.encode())
    if at < 0:
        return None
    return off + at


def cmd_find(args):
    name = args.const
    addr = find_string(name)
    if addr is None:
        log(f"string {name!r} not in .rodata")
        return 1
    log(f"{name!r} at module 0x{addr:x} (rodata)")
    _, text = seg_bytes("text")
    log("scanning 15.1M instructions for the adrp/add pair that materialises it...")
    hits = scan_adrp_add(text, addr)
    log(f"{len(hits)} code reference(s)")
    syms = load_symbols()
    known = sorted(known_entrypoints())
    import bisect
    for adrp, add, rd in hits:
        i = bisect.bisect_right(known, adrp) - 1
        owner = known[i] if i >= 0 else 0
        nm = syms.get(owner, "")
        log(f"  adrp 0x{adrp:x} add 0x{add:x} -> x{rd}   inside 0x{owner:x} {nm}")
        # The `bl` that consumes it is what registers the constant.
        words = struct.unpack_from("<%dI" % 16, text, add + 4)
        for k, w in enumerate(words):
            if (w & 0xFC000000) == 0x94000000:      # BL
                imm = w & 0x03FFFFFF
                if imm & (1 << 25):
                    imm -= 1 << 26
                tgt = add + 4 + k * 4 + imm * 4
                log(f"      first bl at +0x{k*4+4:x} -> 0x{tgt:x} {syms.get(tgt,'')}")
                break
    return 0


# ---------------------------------------------------------------------------
# the emulator itself
# ---------------------------------------------------------------------------

def build_uc():
    from unicorn import Uc, UC_ARCH_ARM64, UC_MODE_ARM, UC_PROT_READ, UC_PROT_WRITE, \
        UC_PROT_EXEC, UC_HOOK_MEM_UNMAPPED
    from unicorn.arm64_const import UC_ARM64_REG_SP

    uc = Uc(UC_ARCH_ARM64, UC_MODE_ARM)
    prot_map = {"r": UC_PROT_READ, "rw": UC_PROT_READ | UC_PROT_WRITE,
                "rx": UC_PROT_READ | UC_PROT_EXEC}
    for name, fn, off, size, prot in SEGS:
        with open(os.path.join(VENDOR, fn), "rb") as f:
            blob = f.read()
        # bss is dumped short (0x680000 of 0x219e000); the rest is zero by definition.
        mapped = (size + 0xFFF) & ~0xFFF
        uc.mem_map(BASE + off, mapped, prot_map[prot])
        uc.mem_write(BASE + off, blob[:size])
        log(f"  map {name:6} 0x{BASE+off:x}+0x{mapped:x} {prot} ({len(blob)/1e6:.1f} MB in)")

    uc.mem_map(STACK_BASE, STACK_SZ)
    uc.mem_map(HEAP_BASE, HEAP_SZ)
    uc.reg_write(UC_ARM64_REG_SP, STACK_BASE + STACK_SZ - 0x1000)

    # Anything the module touches that we did not map -- a TLS slot, a kernel page --
    # gets backed on demand rather than ending the run. Without this, emulation dies
    # on the first unmapped read and tells us nothing.
    def on_unmapped(uc, access, address, size, value, user):
        try:
            uc.mem_map(address & ~0xFFFFF, 0x100000)
            return True
        except Exception:
            return False

    uc.hook_add(UC_HOOK_MEM_UNMAPPED, on_unmapped)
    return uc


def read_cstr(uc, addr, limit=512):
    out = bytearray()
    while len(out) < limit:
        try:
            chunk = uc.mem_read(addr + len(out), 32)
        except Exception:
            break
        z = chunk.find(b"\x00")
        if z >= 0:
            out += chunk[:z]
            break
        out += chunk
    return out.decode("latin-1")


def cmd_consts(args):
    """Emulate the registering function; record every constant it registers.

    The register call is hooked by ADDRESS (found by `find`), and each call carries
    x1 = name string, x2 = value. Returning immediately from it keeps the registering
    function walking its list instead of descending into the game's allocator.
    """
    from unicorn import UC_HOOK_CODE, UC_SECOND_SCALE, UcError
    from unicorn.arm64_const import UC_ARM64_REG_X1, UC_ARM64_REG_X2, UC_ARM64_REG_LR, \
        UC_ARM64_REG_PC

    os.makedirs(WORK, exist_ok=True)
    entry = BASE + int(args.entry, 16)
    reg = BASE + int(args.register, 16)
    out_path = os.path.join(WORK, "consts-1305.csv")
    uc = build_uc()
    log(f"entry 0x{entry:x}  register 0x{reg:x}")

    seen = []
    state = {"n": 0, "last": time.time()}
    out = open(out_path, "w", buffering=1)

    def hook(uc, address, size, user):
        state["n"] += 1
        if address == reg:
            x1 = uc.reg_read(UC_ARM64_REG_X1)
            x2 = uc.reg_read(UC_ARM64_REG_X2)
            name = read_cstr(uc, x1)
            if name:
                out.write('CONST,0x%010X,"%s",0x%X\n' % (hash40(name), name, x2))
                seen.append(name)
            uc.reg_write(UC_ARM64_REG_PC, uc.reg_read(UC_ARM64_REG_LR))
        if state["n"] % 2000000 == 0:
            log(f"  {state['n']/1e6:.0f}M instructions, {len(seen)} constants")

    uc.hook_add(UC_HOOK_CODE, hook)
    try:
        uc.emu_start(entry, 0, timeout=args.timeout * UC_SECOND_SCALE,
                     count=args.max_insns)
    except UcError as e:
        log(f"stopped: {e}")
    out.close()
    log(f"{len(seen)} constants -> {out_path} ({state['n']} instructions)")
    return 0


def cmd_indirect(args):
    """The long sweep: emulate every known entry point, record indirect call targets.

    A function reached only through a vtable or a function pointer is not a `bl`
    target, which is the documented ceiling on tools/disasm --callees. Emulating a
    function with real .data behind it resolves `blr Xn` to the address the game
    itself would call. Most emulations fault early -- no `this`, no allocator -- and
    that is fine: what is wanted is the targets reached before the fault.

    Resumable at entry-point granularity: done addresses are appended to a progress
    file and skipped on restart.
    """
    from unicorn import UC_HOOK_CODE, UC_SECOND_SCALE, UcError
    from unicorn.arm64_const import UC_ARM64_REG_X0, UC_ARM64_REG_SP, UC_ARM64_REG_LR

    os.makedirs(WORK, exist_ok=True)
    prog_path = os.path.join(WORK, "indirect.progress")
    out_path = os.path.join(WORK, "indirect-targets.txt")
    done = set()
    if os.path.exists(prog_path):
        with open(prog_path) as f:
            for ln in f:
                ln = ln.strip()
                if ln:
                    done.add(int(ln, 16))
        log(f"resuming: {len(done)} entry points already swept")

    seeds = sorted(known_entrypoints() | set(
        int(l, 16) for l in open(os.path.join(WORK, "ptr-entrypoints.txt"))
    )) if os.path.exists(os.path.join(WORK, "ptr-entrypoints.txt")) else sorted(known_entrypoints())
    todo = [a for a in seeds if a not in done]
    log(f"{len(seeds)} seeds, {len(todo)} to go")

    uc = build_uc()
    _, text = seg_bytes("text")
    targets = {}
    out = open(out_path, "a", buffering=1)
    prog = open(prog_path, "a", buffering=1)

    cur = {"pc": 0}

    def hook(uc, address, size, user):
        # Read the instruction about to run; BLR/BR name their target in a register.
        off = address - BASE
        if not (0 <= off < TEXT_SZ - 4):
            return
        w = struct.unpack_from("<I", text, off)[0]
        if (w & 0xFFFFFC1F) == 0xD63F0000 or (w & 0xFFFFFC1F) == 0xD61F0000:  # BLR / BR Xn
            rn = (w >> 5) & 0x1F
            try:
                from unicorn.arm64_const import UC_ARM64_REG_X0 as R0
                tgt = uc.reg_read(R0 + rn) if rn < 29 else 0
            except Exception:
                return
            if BASE <= tgt < BASE + TEXT_SZ and (tgt - BASE) % 4 == 0:
                t = tgt - BASE
                if t not in targets:
                    targets[t] = cur["pc"]
                    out.write("0x%010x from 0x%010x\n" % (t, cur["pc"]))

    uc.hook_add(UC_HOOK_CODE, hook)

    t0 = time.time()
    for i, a in enumerate(todo):
        cur["pc"] = a
        uc.reg_write(UC_ARM64_REG_SP, STACK_BASE + STACK_SZ - 0x10000)
        uc.reg_write(UC_ARM64_REG_X0, HEAP_BASE + 0x10000)   # a plausible `this`
        uc.reg_write(UC_ARM64_REG_LR, BASE)                  # return lands in .text[0]
        try:
            uc.emu_start(BASE + a, BASE, count=args.budget, timeout=UC_SECOND_SCALE // 2)
        except UcError:
            pass
        except Exception:
            pass
        prog.write("0x%010x\n" % a)
        if (i + 1) % 500 == 0:
            el = time.time() - t0
            rate = (i + 1) / el
            log(f"  {i+1}/{len(todo)} swept ({rate:.0f}/s), {len(targets)} indirect targets, "
                f"eta {(len(todo)-i-1)/rate/60:.0f} min")
    out.close()
    prog.close()
    log(f"done: {len(targets)} indirect targets -> {out_path}")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="cmd", required=True)

    sub.add_parser("ptrs", help="harvest function pointers from .data/.bss")

    f = sub.add_parser("find", help="locate the registration call site by xref")
    f.add_argument("--const", default="FIGHTER_PAD_ATTACK_TRIGGER")

    c = sub.add_parser("consts", help="emulate the registering function")
    c.add_argument("--entry", required=True, help="registering function, module-relative hex")
    c.add_argument("--register", required=True, help="register function, module-relative hex")
    c.add_argument("--timeout", type=int, default=3600)
    c.add_argument("--max-insns", type=int, default=0)

    i = sub.add_parser("indirect", help="sweep every entry point for indirect targets")
    i.add_argument("--budget", type=int, default=20000, help="instructions per entry point")

    args = ap.parse_args()
    return {"ptrs": cmd_ptrs, "find": cmd_find, "consts": cmd_consts,
            "indirect": cmd_indirect}[args.cmd](args) or 0


if __name__ == "__main__":
    sys.exit(main())
