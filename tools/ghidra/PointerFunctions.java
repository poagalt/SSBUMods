// Give every code address that is only ever reached through a POINTER a function of its own.
//
// WHY THIS EXISTS. Auto-analysis creates functions where flow goes (calls, the ELF's STT_FUNC
// seeds, and the targets of the module's RELATIVE relocations into .text -- tools/rea-elf.py).
// What it does not do is start a function at an address that code only takes the address OF.
// The lua2cpp NROs build every status callback that way:
//
//     adrp x1, 0x7100033000 ; add x1, x1, #0x6a0      -> lib::L2CValue::L2CValue(v, &LAB_71000336a0)
//     bl   lua2cpp::L2CFighterCommon::sub_shift_status_main
//
// and register every ACMD/expression body the same way (sv_set_function_hash(this, &LAB_, hash)).
// Those targets are referenced (Ghidra records a PARAM or DATA reference to them), but they are
// left as bare LAB_ code, sometimes not even disassembled, so CodeIndex.java had nothing to
// decompile: the status main loops, the very code that decides what happens next, were missing.
//
// WHAT IT DOES, per program, in rounds until a round finds nothing new:
//   1. every reference destination inside an executable, initialized block whose reference is a
//      pointer-taking kind (DATA, PARAM, DATA_IND, INDIRECTION) or a call, and which no function
//      contains yet, is a candidate. Jumps are not: a jump into bare code is a label of code that
//      some other candidate's function reaches by flow. READ/WRITE are not: what code loads from
//      .text is a literal, not an entry point.
//   2. candidates are taken in address order, re-checked (an earlier one this round may have
//      absorbed it), disassembled if Ghidra never did, and given a function (CreateFunctionCmd).
//   3. analyzeChanges() runs the program's own analyzers over just the new code, which is what
//      adds the adrp/add references inside it -- the next round's candidates.
// A target that already sits INSIDE a function (an exception landing pad, a `br` label, a
// mis-bounded function) is never split; it is only counted, and listed, as "inside".
//
// Run WRITABLE (no -readOnly) against an analysed project, so the new functions are saved into it,
// then export with CodeIndex.java (CI_KNOWN makes that incremental). tools/code-index.sh does both:
//   bash tools/code-index.sh pointers nro|main|<program.elf>
//
//   CI_OUT   output directory; writes <program>/pointer-functions.tsv (every function this pass ever
//            made, from its ANALYSIS bookmarks in category "PointerFunctions", plus this run's
//            candidates it did not make) and pointer-functions.txt (this run's counts)
//   PF_ROUNDS  maximum rounds (default 12)
//   PF_STRICT  1/0; default on only for a program linked below 4 GB (main): a candidate must then
//              be a pointer the code builds (adr, adrp+add) or a data word holds, see provenance()
import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.cmd.function.CreateFunctionCmd;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.*;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;

public class PointerFunctions extends GhidraScript {
    private static final String BOOKMARK = "PointerFunctions";
    private Path dir;
    private final StringBuilder summary = new StringBuilder();

    private static String hex(Address a) {
        return a == null ? "" : "0x" + Long.toHexString(a.getOffset());
    }

    private void note(String line) throws IOException {
        println(line);
        summary.append(line).append('\n');
        Files.write(dir.resolve("pointer-functions.txt"), summary.toString().getBytes(StandardCharsets.UTF_8));
    }

    // The word before a candidate, as the reason it may start a function: padding, or the last
    // instruction of whatever precedes it (ret, b, br, brk, nop, udf, or a bl to something that does
    // not return). Anything else is "mid": the candidate is a label inside running code.
    private static String boundary(int w) {
        if (w == 0) {
            return "zero";
        }
        if (w == 0xD65F03C0) {
            return "ret";
        }
        if ((w & 0xFC000000) == 0x14000000) {
            return "b";
        }
        if ((w & 0xFC000000) == 0x94000000) {
            return "bl";
        }
        if ((w & 0xFFFFFC1F) == 0xD61F0000) {
            return "br";
        }
        if ((w & 0xFFE0001F) == 0xD4200000) {
            return "brk";
        }
        if (w == 0xD503201F) {
            return "nop";
        }
        if ((w & 0xFFFF0000) == 0) {
            return "udf";
        }
        return "mid";
    }

    // STRICT mode's test, that some reference to `t` is a pointer the program really builds:
    //   from code, an `adr` to t, or `adrp` page(t) plus `add` lo12(t) on that register, within the
    //     1 KB before the referencing instruction (or the 8 bytes after it), and t not mid-code;
    //   from data, an 8-byte word equal to t, with t after padding, ret, br, brk, udf or nop.
    // Null when t passes, else the reason it does not. Measured on main 2026-10-18 before this test
    // existed: of 7,939 functions made from main's references, 7,587 were adrp+add-formed, 50 were
    // held only by exact pointer words in .data, and ~300 were neither: register values Ghidra got
    // wrong (`stp x8, xzr, [x19, #0xf8]` "pointing" at 0x2e6ea00) and small immediates, 174 of them
    // below 0x10000, carving mid-function curl code into 4-byte "functions".
    private String provenance(Address t, ReferenceManager rm, AddressSet exec) throws Exception {
        ghidra.program.model.mem.Memory mem = currentProgram.getMemory();
        String prev = t.getOffset() >= 4 && exec.contains(t.subtract(4)) ? boundary(mem.getInt(t.subtract(4))) : "zero";
        boolean code = false, data = false;
        for (Reference ref : rm.getReferencesTo(t)) {
            if (!pointerKind(ref.getReferenceType())) {
                continue;
            }
            Address s = ref.getFromAddress();
            if (exec.contains(s)) {
                code = true;
                if (!prev.equals("mid") && formed(t.getOffset(), s, exec, mem)) {
                    return null;
                }
            }
            else {
                data = true;
                boolean clean = !prev.equals("mid") && !prev.equals("b") && !prev.equals("bl");
                if ((s.getOffset() & 7) == 0 && clean && mem.getLong(s) == t.getOffset()) {
                    return null;
                }
            }
        }
        if (prev.equals("mid")) {
            return "rejected (strict): mid-code (previous instruction is not a boundary)";
        }
        if (code) {
            return "rejected (strict): not built by adr/adrp+add near any referencing instruction";
        }
        return data ? "rejected (strict): no exact pointer word after a clean boundary (prev " + prev + ")"
            : "rejected (strict): no pointer reference";
    }

    private static boolean formed(long t, Address s, AddressSet exec, ghidra.program.model.mem.Memory mem)
            throws Exception {
        long page = t & ~0xFFFL, low = t & 0xFFF;
        Set<Integer> adrpRegs = new HashSet<>();
        List<Integer> addRn = new ArrayList<>();
        long from = Math.max(0, (s.getOffset() & ~3L) - 0x400), to = (s.getOffset() & ~3L) + 8;
        for (long a = from; a <= to; a += 4) {
            Address aa = s.getNewAddress(a);
            if (!exec.contains(aa) || !exec.contains(aa.add(3))) {
                continue;
            }
            int w = mem.getInt(aa);
            if ((w & 0x9F000000) == 0x10000000) {                 // adr
                long imm = (((w >>> 5) & 0x7FFFF) << 2) | ((w >>> 29) & 3);
                if ((imm & (1L << 20)) != 0) {
                    imm -= 1L << 21;
                }
                if (a + imm == t) {
                    return true;
                }
            }
            else if ((w & 0x9F000000) == 0x90000000) {            // adrp
                long imm = ((((long) (w >>> 5) & 0x7FFFF) << 2) | ((w >>> 29) & 3)) << 12;
                if ((imm & (1L << 32)) != 0) {
                    imm -= 1L << 33;
                }
                if ((a & ~0xFFFL) + imm == page) {
                    adrpRegs.add(w & 0x1F);
                }
            }
            else if ((w & 0x7F800000) == 0x11000000 && ((w >>> 22) & 1) == 0 && ((w >>> 10) & 0xFFF) == low) {
                addRn.add((w >>> 5) & 0x1F);                        // add #imm12, unshifted
            }
        }
        for (int rn : addRn) {
            if (adrpRegs.contains(rn)) {
                return true;
            }
        }
        return low == 0 && !adrpRegs.isEmpty();
    }

    private static boolean pointerKind(RefType t) {
        if (t.isCall()) {
            return true;
        }
        return t == RefType.DATA || t == RefType.PARAM || t == RefType.DATA_IND || t == RefType.INDIRECTION;
    }

    @Override
    public void run() throws Exception {
        String out = System.getenv("CI_OUT");
        if (out == null || out.isEmpty()) {
            throw new IllegalArgumentException("CI_OUT is not set");
        }
        int maxRounds = 12;
        String r = System.getenv("PF_ROUNDS");
        if (r != null && !r.isEmpty()) {
            maxRounds = Integer.parseInt(r);
        }
        dir = Paths.get(out, currentProgram.getName().replaceAll("\\.elf$", ""));
        Files.createDirectories(dir);

        AddressSpace space = currentProgram.getAddressFactory().getDefaultAddressSpace();
        FunctionManager fm = currentProgram.getFunctionManager();
        ReferenceManager rm = currentProgram.getReferenceManager();
        Listing listing = currentProgram.getListing();
        BookmarkManager bm = currentProgram.getBookmarkManager();
        AddressSet exec = new AddressSet();
        for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
            if (b.isExecute() && b.isInitialized() && b.getStart().getAddressSpace().equals(space)) {
                exec.add(b.getStart(), b.getEnd());
            }
        }
        // STRICT for a module linked low (main, at 0x0): there every small immediate and every
        // offset Ghidra could resolve is also a .text address, and it made DATA/PARAM references of
        // them (main's index: 1036, 1040, 1041 ... as "pointers"). A candidate must then pass
        // provenance(): built by adr/adrp+add, or held by an exact pointer word. The rest are
        // refused, and listed. An NRO at 0x7100000000 has no such aliasing (every one of its
        // 110,996 &LAB_/&DAT_ code pointers lands on a new entry); PF_STRICT=1/0 overrides.
        String sv = System.getenv("PF_STRICT");
        boolean strict = sv == null || sv.isEmpty() ? currentProgram.getImageBase().getOffset() < 0x100000000L
            : sv.equals("1");
        long t0 = System.currentTimeMillis();
        int before = fm.getFunctionCount();
        note("program " + currentProgram.getName() + "  functions before " + before + "  executable " + exec
            + (strict ? "  STRICT" : ""));

        // Every candidate ever considered, with its outcome; written as one table at the end.
        Map<Address, String> outcome = new TreeMap<>();
        Map<Address, String> why = new HashMap<>();
        Map<String, Integer> tally = new TreeMap<>();
        int created = 0;
        for (int round = 1; round <= maxRounds; round++) {
            monitor.checkCancelled();
            TreeMap<Address, Reference> cand = new TreeMap<>();
            AddressIterator it = rm.getReferenceDestinationIterator(exec, true);
            while (it.hasNext()) {
                Address to = it.next();
                if (outcome.containsKey(to) || fm.getFunctionAt(to) != null) {
                    continue;
                }
                Reference pick = null;
                for (Reference ref : rm.getReferencesTo(to)) {
                    if (pointerKind(ref.getReferenceType())) {
                        pick = ref;
                        break;
                    }
                }
                if (pick == null) {
                    continue;
                }
                Function in = fm.getFunctionContaining(to);
                if (in != null) {
                    if (!in.getEntryPoint().equals(to)) {
                        outcome.put(to, "inside " + hex(in.getEntryPoint()));
                        why.put(to, hex(pick.getFromAddress()) + "\t" + pick.getReferenceType());
                        tally.merge("inside an existing function (not split)", 1, Integer::sum);
                    }
                    continue;
                }
                cand.put(to, pick);
            }
            int made = 0;
            // HIGHEST ADDRESS FIRST. A function's body stops at another function's entry, so once
            // every later candidate exists, a call Ghidra wrongly thinks returns (an assert wrapper
            // ending in `bl <noreturn>`) cannot fall through into the next candidate and swallow it.
            // Ascending order measured on lua2cpp_packun: 51 pointer targets absorbed that way.
            for (Map.Entry<Address, Reference> e : cand.descendingMap().entrySet()) {
                monitor.checkCancelled();
                Address a = e.getKey();
                Reference ref = e.getValue();
                why.put(a, hex(ref.getFromAddress()) + "\t" + ref.getReferenceType());
                String res;
                Function in = fm.getFunctionContaining(a);
                if (in != null) {
                    res = in.getEntryPoint().equals(a) ? "exists" : "absorbed by " + hex(in.getEntryPoint());
                    tally.merge(in.getEntryPoint().equals(a) ? "already a function" : "absorbed by another new function", 1, Integer::sum);
                }
                else if ((a.getOffset() & 3) != 0) {
                    res = "misaligned";
                    tally.merge("misaligned", 1, Integer::sum);
                }
                else if (listing.getDefinedDataContaining(a) != null) {
                    res = "defined data";
                    tally.merge("defined data", 1, Integer::sum);
                }
                else {
                    Instruction ins = listing.getInstructionContaining(a);
                    if (ins != null && !ins.getAddress().equals(a)) {
                        res = "mid-instruction";
                        tally.merge("mid-instruction", 1, Integer::sum);
                    }
                    else if (strict && (res = provenance(a, rm, exec)) != null) {
                        tally.merge(res.replaceAll(" \\(prev .*", ""), 1, Integer::sum);
                    }
                    else {
                        boolean disassembled = false;
                        if (ins == null) {
                            new DisassembleCommand(a, null, true).applyTo(currentProgram, monitor);
                            disassembled = true;
                        }
                        if (listing.getInstructionAt(a) == null) {
                            res = "does not disassemble";
                            tally.merge("does not disassemble", 1, Integer::sum);
                        }
                        else {
                            CreateFunctionCmd cmd = new CreateFunctionCmd(a);
                            if (cmd.applyTo(currentProgram, monitor) && fm.getFunctionAt(a) != null) {
                                res = "created round " + round + (disassembled ? " (disassembled)" : "");
                                tally.merge("created", 1, Integer::sum);
                                made++;
                                // The durable record, kept IN the project: a later run (which finds
                                // nothing new) still lists every function this pass ever made.
                                bm.setBookmark(a, BookmarkType.ANALYSIS, BOOKMARK,
                                    res + "; " + hex(ref.getFromAddress()) + " " + ref.getReferenceType());
                            }
                            else {
                                res = "create failed: " + cmd.getStatusMsg();
                                tally.merge("create failed", 1, Integer::sum);
                            }
                        }
                    }
                }
                outcome.put(a, res);
            }
            created += made;
            note(String.format("round %d: %d candidates, %d functions created (%ds)", round, cand.size(), made,
                (System.currentTimeMillis() - t0) / 1000));
            if (made == 0) {
                break;
            }
            // The program's own analyzers, over the new code only: references, switch tables, stack.
            analyzeChanges(currentProgram);
            note(String.format("round %d: analysis of the new code done (%ds)", round, (System.currentTimeMillis() - t0) / 1000));
        }

        // Every function this pass has EVER created (its bookmarks), plus this run's other outcomes.
        Iterator<Bookmark> bit = bm.getBookmarksIterator(BookmarkType.ANALYSIS);
        int ever = 0;
        while (bit.hasNext()) {
            Bookmark b = bit.next();
            if (!BOOKMARK.equals(b.getCategory())) {
                continue;
            }
            ever++;
            Address a = b.getAddress();
            if (!outcome.containsKey(a) || !outcome.get(a).startsWith("created")) {
                String[] c = b.getComment().split("; ", 2);
                String[] fromType = c.length > 1 ? c[1].split(" ", 2) : new String[] { "", "" };
                outcome.put(a, c[0]);
                why.put(a, fromType[0] + "\t" + (fromType.length > 1 ? fromType[1] : ""));
            }
        }
        try (BufferedWriter w = new BufferedWriter(new OutputStreamWriter(
                new FileOutputStream(dir.resolve("pointer-functions.tsv").toFile()), StandardCharsets.UTF_8))) {
            w.write("address\tresult\tfirst_ref_from\tref_type\tfunction_now\tprev_insn\n");
            for (Map.Entry<Address, String> e : outcome.entrySet()) {
                Function f = fm.getFunctionAt(e.getKey());
                // What precedes a new entry: a ret/b/brk is a clean boundary; anything else means the
                // previous code falls into this entry (a noreturn call Ghidra does not know of, or a
                // pointer to a label inside a function), which is worth a look.
                Instruction prev = listing.getInstructionBefore(e.getKey());
                String pv = "";
                if (prev != null && prev.getAddress().add(prev.getLength()).equals(e.getKey())) {
                    Function pf = fm.getFunctionContaining(prev.getAddress());
                    pv = prev.getMnemonicString() + (pf == null ? "" : " in " + hex(pf.getEntryPoint()));
                }
                w.write(hex(e.getKey()) + "\t" + e.getValue() + "\t" + why.getOrDefault(e.getKey(), "\t") + "\t"
                    + (f == null ? "" : f.getName(true)) + "\t" + pv + "\n");
            }
        }
        for (Map.Entry<String, Integer> e : tally.entrySet()) {
            note("  " + e.getKey() + ": " + e.getValue());
        }
        note("created by this pass, all runs (bookmarks \"" + BOOKMARK + "\"): " + ever);
        note(String.format("functions before %d, after %d, created %d, in %ds", before, fm.getFunctionCount(), created,
            (System.currentTimeMillis() - t0) / 1000));
    }
}
