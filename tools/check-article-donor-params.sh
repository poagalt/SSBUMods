#!/usr/bin/env bash
# A cloned article must not run native code that reads its DONOR fighter's param entry.
#
# WHY THIS EXISTS. `clone_weapon` appends another fighter's article descriptor to Pikachu's, and
# the descriptor's callbacks are pointers into the game's own code. That code is compiled per
# donor, so some of it reads the donor's entry in `FighterParamAccessor2` with the fighter kind
# CONSTANT-FOLDED IN. That entry only exists while the donor is in the match, and Piranha Plant
# never is -- so the spikeball clone died at `main + 0x3476e44` reading `[null + 0x138]`, ten boots
# out of ten, while the same build with the clone off loaded 42 out of 42.
#
# docs/the-article-path.md has the whole derivation. The short version:
#
#   * `FighterParamAccessor2` holds an array of 56-byte entries at `this + 0x70`, indexed by
#     FIGHTER KIND. Proven from `FighterParamAccessor2__hit_target_no_impl` (0x20a8080), which is
#     nine instructions: `mov w9,#0x38` / `smaddl x8,w8,w9,x0` / `ldr x0,[x8,#0x70]`, with a
#     `cmp w1,#0x4c` / `csel #0x4b` that maps NANA onto POPO -- the Ice Climbers sharing one entry
#     is what proves the index is a fighter kind and not an arbitrary field offset.
#   * So the entry for fighter kind K is at `0x70 + K*0x38`, and `0x1228` -- the offset in the
#     crash -- is exactly K = 0x51 = FIGHTER_KIND_PACKUN, remainder zero.
#   * The faulting function is vtable SLOT 3 of the weapon class for WEAPON_KIND_PACKUN_SPIKEBALL.
#     The caller is `ldr x0,[x19,#0x3bb0]` / `ldr x8,[x0]` / `ldr x8,[x8,#0x18]` / `blr x8` at
#     `main + 0x33a07b8`, whose return address is the LR in the crash report.
#
# AND IT IS PER-ARTICLE, WHICH IS THE POINT. Chrom's sword guards the identical read on the owner's
# kind and falls through with zeroed locals when the owner is neither Chrom nor Roy. Five of the
# seven articles this port has wanted are clean across all 114 vtable slots; the spikeball and
# Incineroar's rope are not. Choosing a clean article is the whole fix, and this is the check that
# says whether the one currently wired up is clean.
#
# WHAT IT DOES NOT DO. It proves nothing about the article's ASSETS, its params, or whether the
# move that uses it is written. It answers exactly one question: does the donor's own weapon class
# dereference an entry that will be null here.
#
# SELF-TEST. Two controls run every time, because a scan that silently stops matching would
# otherwise report everything clean: PACKUN_SPIKEBALL must come back UNGUARDED and CHROM_SWORD must
# come back clean. If either control flips, the method broke and the verdicts are not trustworthy.
#
#   bash tools/check-article-donor-params.sh
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"

A="$ROOT/pikachu-moveset/plugin/src/articles/mod.rs"
TEXT="$ROOT/vendor/main/rat-main-text.bin"
DATA="$ROOT/vendor/main/rat-main-data.bin"
SYMS="$ROOT/vendor/main/symbols-merged.txt"
CONSTS="$ROOT/vendor/main/consts-1305.csv"
BASES="$ROOT/vendor/main/rat-memserve.txt"

# vendor/ is gitignored, so a fresh clone cannot run this. Absence is not a failure -- it is a
# clone without the dumps, and docs/reading-the-games-code.md rebuilds them.
for f in "$TEXT" "$DATA" "$SYMS" "$CONSTS" "$BASES"; do
    if [ ! -s "$f" ]; then
        echo "  skip  ${f#$ROOT/} is missing -- see docs/reading-the-games-code.md to rebuild vendor/"
        exit 0
    fi
done

PY=""
for c in py python3 python; do command -v "$c" >/dev/null 2>&1 && { PY="$c"; break; }; done
[ -n "$PY" ] || { echo "  skip  no python on PATH"; exit 0; }

"$PY" - "$A" "$TEXT" "$DATA" "$SYMS" "$CONSTS" "$BASES" <<'PY'
import re, struct, sys
art_rs, text_p, data_p, syms_p, consts_p, bases_p = sys.argv[1:7]

text = open(text_p, 'rb').read()
data = open(data_p, 'rb').read()
TEXT_SPAN = len(text)

# ---- where the dump was mapped -----------------------------------------------------------------
# Both blobs came off the RUNNING process, so every pointer in .data is an absolute runtime
# address and has to be un-relocated before it means anything. vendor/main/rat-memserve.txt is the
# record of that run's bases, written by the same dumper. The section SIZES in it are the check
# that the bases belong to these blobs: if the dumps are ever regenerated without the bases file
# being updated, the sizes disagree and this skips instead of reading garbage.
reg = {}
for l in open(bases_p):
    f = l.split()
    if len(f) == 3 and f[0] in ('text', 'rodata', 'data', 'bss'):
        try: reg[f[0]] = (int(f[1], 16), int(f[2], 16))
        except ValueError: pass
if 'text' not in reg or 'data' not in reg:
    print("  skip  %s has no text/data bases" % bases_p.split('/')[-1]); raise SystemExit(0)
TEXTBASE, tsz = reg['text']
DATABASE, dsz = reg['data']
if tsz != len(text) or dsz != len(data):
    print("  skip  the dumps do not match the recorded bases (text %#x vs %#x, data %#x vs %#x) "
          "-- regenerate both together" % (tsz, len(text), dsz, len(data)))
    raise SystemExit(0)

def dq(off):
    return struct.unpack_from('<Q', data, off)[0] if 0 <= off <= len(data)-8 else 0
def as_data(v):
    return v - DATABASE if DATABASE <= v < DATABASE + len(data) else None
def as_text(v):
    return v - TEXTBASE if TEXTBASE <= v < TEXTBASE + TEXT_SPAN else None

# ---- the singleton, from the symbol table rather than a literal ---------------------------------
SING = None
for l in open(syms_p):
    if 'SingletonIN3app21FighterParamAccessor2EE9instance_' in l:
        SING = int(l.split()[0], 16); break
if SING is None:
    print("  skip  FighterParamAccessor2's singleton is not in symbols-merged.txt"); raise SystemExit(0)

# ---- constant tables ---------------------------------------------------------------------------
fkind, wkind = {}, {}
for l in open(consts_p):
    m = re.match(r'CONST,0x[0-9A-F]+,"FIGHTER_KIND_([A-Z0-9_]+)",0x([0-9a-fA-F]+),', l)
    if m and not m.group(1).startswith('BASE'): fkind.setdefault(m.group(1), int(m.group(2), 16))
    m = re.match(r'CONST,0x[0-9A-F]+,"WEAPON_KIND_([A-Z0-9_]+)",0x([0-9a-fA-F]+),', l)
    if m: wkind.setdefault('WEAPON_KIND_' + m.group(1), int(m.group(2), 16))

# ---- the weapon class table, located by structure ----------------------------------------------
# table[weapon_kind] -> a singleton instance whose first field is its vtable address point. Found
# by sliding a 615-entry window (WEAPON_KIND_* runs 0..614) and taking the offset where the most
# entries resolve to "a .data pointer whose target is also a .data pointer". That window is
# .data+0x386400 with 612 of 615 resolving, and the runner-up is the same table off by one slot --
# so it is not a coincidence that has to be trusted.
N_KINDS = 615
n = len(data) // 8
V = struct.unpack('<%dQ' % n, data[:n*8])
ok = bytearray(n)
for i, v in enumerate(V):
    d = as_data(v)
    if d is not None and d % 8 == 0 and as_data(V[d // 8]) is not None: ok[i] = 1
best, s = (-1, -1), sum(ok[:N_KINDS])
for i in range(n - N_KINDS):
    if i: s += ok[i+N_KINDS-1] - ok[i-1]
    if s > best[0]: best = (s, i*8)
hits, TBASE = best
if hits < N_KINDS * 0.9:
    print("  skip  could not locate the weapon class table (best window %d/%d)" % (hits, N_KINDS))
    raise SystemExit(0)

def vtable_of(kind):
    h = as_data(dq(TBASE + kind*8))
    if h is None: return None
    return as_data(dq(h))

VTABLE_SLOTS = 114   # uniform across all 522 distinct weapon vtables in this build

# ---- instruction scanning ----------------------------------------------------------------------
def word(a): return struct.unpack_from('<I', text, a)[0] if 0 <= a <= len(text)-4 else 0

def sx(v, bits): return v - (1 << bits) if v >> (bits-1) else v

def scan(fn, donor_off, donor_kind, limit=400):
    """Walk one function to its first `ret`. Return (unguarded_reads, guarded_reads)."""
    page = {}; holds = set(); unguarded = guarded = 0; kind_cmp = False
    a = fn
    for _ in range(limit):
        w = word(a)
        if w == 0xD65F03C0: break                                  # ret
        if (w & 0x9F000000) == 0x90000000:                          # adrp xd, page
            rd = w & 0x1F
            page[rd] = (a & ~0xFFF) + (sx((((w >> 5) & 0x7FFFF) << 2) | ((w >> 29) & 3), 21) << 12)
            holds.discard(rd)
        elif (w & 0xFFC00000) == 0xF9400000:                        # ldr xt, [xn, #imm]
            off = ((w >> 10) & 0xFFF) * 8; rn = (w >> 5) & 0x1F; rt = w & 0x1F
            if rn in page and page[rn] + off == SING:
                holds.add(rt); page.pop(rt, None)
            elif rn in holds and off == donor_off:
                if kind_cmp: guarded += 1
                else: unguarded += 1
                holds.discard(rt)
            else:
                holds.discard(rt); page.pop(rt, None)
        elif (w & 0xFF80001F) in (0x7100001F, 0xF100001F):          # cmp wn/xn, #imm12
            if ((w >> 10) & 0xFFF) == donor_kind: kind_cmp = True
        a += 4
    return unguarded, guarded

def audit(weapon_kind, donor_name):
    dk = fkind.get(donor_name.upper())
    if dk is None: return None
    off = 0x70 + dk * 0x38
    ap = vtable_of(weapon_kind)
    if ap is None: return None
    fns = set(); slot_of = {}
    for i, s in enumerate(range(ap, ap + VTABLE_SLOTS*8, 8)):
        t = as_text(dq(s))
        if t:
            fns.add(t); slot_of.setdefault(t, []).append(i)
    bad = []; good = 0
    for t in sorted(fns):
        u, g = scan(t, off, dk)
        good += g
        if u: bad.append((t, u))
    return {'kind': weapon_kind, 'donor_kind': dk, 'slot_off': off,
            'vtable': ap, 'fns': len(fns), 'unguarded': bad, 'guarded': good,
            'dirty_slots': sorted({sl for t, _ in bad for sl in slot_of.get(t, [])})}

# ---- per-instance neutralisation (src/articles/neutralize.rs), 2026-10-12 ------------------------
# A dirty row may be installed if neutralize.rs gives OUR instances a private vtable with every
# unguarded slot pointed at the engine's default. Read from the Rust, like the article table, and
# checked three ways: the listed slots must cover the scan's dirty slots; the default donor must
# really hold the majority default in each listed slot; and the hook's two addresses must be what
# that file says they are in this dump.
import os, collections
neu_rs = os.path.join(os.path.dirname(art_rs), 'neutralize.rs')
NEUTRALIZED = {}
neu_consts = {}
if os.path.exists(neu_rs):
    nsrc = open(neu_rs, encoding='utf-8').read()
    for host, sl in re.findall(r'\("([a-z_]+)",\s*&\[([0-9,\s]*)\]\)', nsrc):
        NEUTRALIZED[host] = sorted(int(x) for x in sl.replace(' ', '').split(',') if x)
    for name in ('WEAPON_INITIALIZE', 'WEAPON_CLASS_TABLE', 'SPECIALIZER_AT'):
        m = re.search(r'const %s: usize = 0x([0-9a-fA-F]+);' % name, nsrc)
        if m: neu_consts[name] = int(m.group(1), 16)
neu_rc = 0
if NEUTRALIZED:
    # the table, as a MODULE offset: data's offset from text, plus the table's offset in data
    tbl_mod = (DATABASE - TEXTBASE) + TBASE
    if neu_consts.get('WEAPON_CLASS_TABLE') != tbl_mod:
        print("  FAIL neutralize.rs WEAPON_CLASS_TABLE = %#x, but the table is at main+%#x in this dump"
              % (neu_consts.get('WEAPON_CLASS_TABLE', -1), tbl_mod)); neu_rc = 1
    # the initialise virtual must store the class singleton at SPECIALIZER_AT: a 64-bit
    # `str xt, [xn, #imm]` whose imm is SPECIALIZER_AT, within the function
    init, at = neu_consts.get('WEAPON_INITIALIZE'), neu_consts.get('SPECIALIZER_AT')
    found = False
    if init is not None and at is not None and at % 8 == 0:
        for a in range(init, init + 0x1800, 4):
            w = word(a)
            if (w & 0xFFC00000) == 0xF9000000 and ((w >> 10) & 0xFFF) * 8 == at:
                found = True; break
    if not found:
        print("  FAIL neutralize.rs WEAPON_INITIALIZE (%#x) does not store to +%#x in this dump"
              % (init or -1, at or -1)); neu_rc = 1
    # majority default per slot, across every distinct class vtable
    vts = {}
    for k in range(N_KINDS):
        ap2 = vtable_of(k)
        if ap2 is not None: vts[ap2] = [as_text(dq(ap2 + 8*i)) for i in range(VTABLE_SLOTS)]
    majority = {i: collections.Counter(v[i] for v in vts.values()).most_common(1)[0][0] for i in range(VTABLE_SLOTS)}
    dd = wkind.get('WEAPON_KIND_GAMEWATCH_PARACHUTE')
    dvt = vtable_of(dd) if dd is not None else None
    for host, sl in NEUTRALIZED.items():
        for i in sl:
            if dvt is None or as_text(dq(dvt + 8*i)) != majority[i]:
                print("  FAIL neutralize.rs copies %s's slot %d from the parachute, which does not hold the "
                      "majority default there" % (host, i)); neu_rc = 1
    if not neu_rc:
        print("  ok  neutralize.rs: table main+%#x and initialise main+%#x (+%#x store) match this dump; "
              "the parachute holds the default in every listed slot" % (tbl_mod, init, at))

# ---- the article table, read from the Rust so it cannot drift -----------------------------------
src = open(art_rs, encoding='utf-8').read()
rows = re.findall(
    r'Candidate\s*\{\s*host_name:\s*"([^"]+)"\s*,\s*donor:\s*"([^"]+)"\s*,'
    r'\s*donor_article:\s*"([^"]+)"\s*,\s*weapon_kind:\s*"([^"]+)"\s*,'
    r'\s*clean:\s*(true|false)\s*,\s*enabled:\s*(true|false)\s*,'
    # `model_from` was added so Samus's charge shot could borrow Pikachu's electro-ball mesh --
    # her own article has no model anywhere in the archive. This gate does not care which model an
    # article wears (it audits the DONOR's native code against its param entry), but the row shape
    # is load-bearing for the regex, and a tightened `\}` turned the whole table into zero rows and
    # this check into a vacuous pass. Matched and discarded rather than left off the end.
    r'\s*authored:\s*(true|false)\s*,\s*model_from:\s*"([^"]*)"\s*\}', src)
clone_on = bool(re.search(r'^pub const CLONE_ENABLED: bool = true;$', src, re.M))

rc = 0
print("  weapon class table at .data+%#x (%d/%d kinds resolve), %d slots audited per class"
      % (TBASE, hits, N_KINDS, VTABLE_SLOTS))

# ---- controls -----------------------------------------------------------------------------------
ctl_bad = audit(wkind['WEAPON_KIND_PACKUN_SPIKEBALL'], 'packun')
ctl_ok  = audit(wkind['WEAPON_KIND_CHROM_SWORD'], 'chrom')
if not ctl_bad or not ctl_bad['unguarded']:
    print("  FAIL control: PACKUN_SPIKEBALL should read PACKUN's entry unguarded and does not -- "
          "the scan is broken, every verdict below is meaningless"); rc = 1
elif ctl_ok is None or ctl_ok['unguarded']:
    print("  FAIL control: CHROM_SWORD should be clean and is not -- the scan is over-matching"); rc = 1
else:
    print("  ok  controls: spikeball %d unguarded read(s), chrom_sword clean with %d guarded"
          % (sum(u for _, u in ctl_bad['unguarded']), ctl_ok['guarded']))

# ---- the table ----------------------------------------------------------------------------------
if not rows:
    print("  FAIL no Candidate rows found in %s" % art_rs.split('/')[-1]); rc = 1

enabled_dirty = []
for host, donor, donor_article, wkname, claimed, enabled, authored, _model_from in rows:
    k = wkind.get(wkname)
    if k is None:
        print("  FAIL %-12s names %s, which is not a WEAPON_KIND_* in consts-1305.csv" % (host, wkname)); rc = 1; continue
    r = audit(k, donor)
    if r is None:
        print("  FAIL %-12s could not resolve a vtable for %s" % (host, wkname)); rc = 1; continue
    is_clean = not r['unguarded']
    mark = "ok " if is_clean == (claimed == 'true') else "FAIL"
    if mark == "FAIL": rc = 1
    sel = " <== INSTALLED" if (enabled == 'true' and clone_on and is_clean) else ""
    print("  %s  %-12s %-30s donor %-10s entry %#-6x  %-24s%s"
          % (mark, host, wkname, donor, r['slot_off'],
             "clean" if is_clean else "UNGUARDED in %d fn(s)" % len(r['unguarded']), sel))
    if not is_clean:
        for t, u in r['unguarded']:
            print("        %#010x  %d read(s) of %s's entry with no kind check" % (t, u, donor.upper()))
    if is_clean != (claimed == 'true'):
        print("        the table claims clean=%s and the scan says clean=%s" % (claimed, is_clean))
    covered = False
    if host in NEUTRALIZED and not is_clean:
        listed, dirty = set(NEUTRALIZED[host]), set(r['dirty_slots'])
        if dirty <= listed:
            covered = True
            print("        neutralised for our instances: dirty slots %s, listed %s" % (sorted(dirty), sorted(listed)))
        else:
            print("  FAIL %-12s neutralize.rs lists slots %s but the dirty slots are %s -- %s uncovered"
                  % (host, sorted(listed), sorted(dirty), sorted(dirty - listed))); rc = 1
        if listed - dirty:
            print("  note %-12s neutralize.rs also lists %s, which read nothing of the donor's: their real "
                  "behaviour is thrown away" % (host, sorted(listed - dirty)))
    if enabled == 'true' and not is_clean and not covered:
        enabled_dirty.append((host, wkname, donor))

# ---- what is actually wired up ------------------------------------------------------------------
# src/articles/mod.rs filters on `clean` before cloning, so a dirty row being enabled cannot
# actually install -- but it is still a contradiction worth failing on, because it means somebody
# expected an article they are not getting.
if clone_on:
    for host, wkname, donor in enabled_dirty:
        print("  FAIL %-12s is ENABLED but %s reads %s's param entry unguarded -- it will be "
              "SKIPPED at runtime. See docs/the-article-path.md." % (host, wkname, donor.upper()))
        rc = 1
    live = [r[0] for r in rows if r[5] == 'true' and r[4] == 'true']
    if live:
        ids = ", ".join("%s=%#x" % (h, 6 + i) for i, h in enumerate(live))
        print("  ok  clone enabled, %d article(s) install: %s" % (len(live), ids))
    else:
        print("  note CLONE_ENABLED is true but no row is both enabled and clean")
else:
    print("  ok  clone disabled")

raise SystemExit(rc or neu_rc)
PY
rc=$?
exit $rc
