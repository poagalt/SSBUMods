// Export everything a fully analysed program knows, once, as plain files.
//
// WHY THIS EXISTS. `work/ghidra/main-all-100k.c` was decompiled from `.text` alone with
// `-noanalysis`, which is why it finished in 15 minutes and why it cannot answer two kinds of
// question: it holds no string (zero literals across 100,508 functions) and no data, so anything
// reached through a pointer -- a vtable slot, a weapon-kind spawn table, a status-function table --
// has no visible caller. `tools/rea-elf.py` builds ELFs that carry `.rodata`/`.data` with the
// module's own relocations applied, and a full auto-analysis of one (50 min for `main`) recovers
// those references. REA throws that analysis away at the end of every session; this script keeps
// what it found, as files `tools/code-index.py` loads into SQLite.
//
// Run read-only against a project that has already been analysed (`tools/code-index.sh`):
//   analyzeHeadless <dir> <project> -process <program> -noanalysis -readOnly -postScript CodeIndex.java
//
//   CI_OUT    output directory; one subdirectory per program
//   CI_DECOMP "0" to skip decompilation (the inventories alone take a minute or two)
//
// Writes, per program:
//   functions.tsv  entry  end  bytes  name  qualified_name  thunk_target  signature
//   symbols.tsv    address  name  qualified_name  type  source  primary
//   refs.tsv       from  to  type  from_function_entry      (memory addresses only; no stack/register)
//   strings.tsv    address  length  value                    (tab, newline and backslash escaped)
//   decomp.c       every function, `/* ==== addr  name ==== */`-delimited like the older corpus
//   summary.txt    counts and timings
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.app.decompiler.parallel.*;
import ghidra.program.model.address.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;

public class CodeIndex extends GhidraScript {
    private Path dir;
    private final StringBuilder summary = new StringBuilder();

    private BufferedWriter open(String name) throws IOException {
        return new BufferedWriter(new OutputStreamWriter(
            new FileOutputStream(dir.resolve(name).toFile()), StandardCharsets.UTF_8), 1 << 20);
    }

    private static String esc(String s) {
        if (s == null) return "";
        StringBuilder b = new StringBuilder(s.length());
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            switch (c) {
                case '\\': b.append("\\\\"); break;
                case '\t': b.append("\\t"); break;
                case '\n': b.append("\\n"); break;
                case '\r': b.append("\\r"); break;
                default:
                    if (c < 0x20) b.append(String.format("\\x%02x", (int) c)); else b.append(c);
            }
        }
        return b.toString();
    }

    private static String hex(Address a) {
        return a == null ? "" : "0x" + Long.toHexString(a.getOffset());
    }

    // The program's own address space only. Ghidra also maps the ELF's non-loaded sections
    // (.symtab, .strtab, the headers) as OTHER blocks, and a string search that reports every
    // symbol name a second time from `.strtab` is noise, not data.
    private AddressSpace space;
    private boolean real(Address a) {
        return a != null && a.getAddressSpace().equals(space);
    }

    private void note(String line) throws IOException {
        println(line);
        summary.append(line).append('\n');
        Files.write(dir.resolve("summary.txt"), summary.toString().getBytes(StandardCharsets.UTF_8));
    }

    @Override
    public void run() throws Exception {
        String out = System.getenv("CI_OUT");
        if (out == null || out.isEmpty()) throw new IllegalArgumentException("CI_OUT is not set");
        dir = Paths.get(out, currentProgram.getName().replaceAll("\\.elf$", ""));
        Files.createDirectories(dir);
        boolean decomp = !"0".equals(System.getenv("CI_DECOMP"));
        space = currentProgram.getAddressFactory().getDefaultAddressSpace();
        FunctionManager fm = currentProgram.getFunctionManager();
        long t0 = System.currentTimeMillis();
        note("program " + currentProgram.getName() + "  image base " + currentProgram.getImageBase());

        // ---- functions ---------------------------------------------------------------------
        List<Function> fns = new ArrayList<>();
        try (BufferedWriter w = open("functions.tsv")) {
            w.write("entry\tend\tbytes\tname\tqualified_name\tthunk_target\tsignature\n");
            for (Function f : fm.getFunctions(true)) {
                if (f.isExternal()) continue;
                fns.add(f);
                Function t = f.isThunk() ? f.getThunkedFunction(false) : null;
                w.write(hex(f.getEntryPoint()) + "\t" + hex(f.getBody().getMaxAddress()) + "\t"
                    + f.getBody().getNumAddresses() + "\t" + esc(f.getName()) + "\t"
                    + esc(f.getSymbol().getName(true)) + "\t"
                    + (t == null ? "" : hex(t.getEntryPoint())) + "\t"
                    + esc(f.getPrototypeString(false, false)) + "\n");
            }
        }
        note("functions " + fns.size());

        // ---- symbols (every stored name, including the mangled originals the demangler kept) --
        int nsym = 0;
        try (BufferedWriter w = open("symbols.tsv")) {
            w.write("address\tname\tqualified_name\ttype\tsource\tprimary\n");
            SymbolIterator it = currentProgram.getSymbolTable().getAllSymbols(false);
            while (it.hasNext()) {
                Symbol s = it.next();
                Address a = s.getAddress();
                if (!real(a)) continue;
                w.write(hex(a) + "\t" + esc(s.getName()) + "\t" + esc(s.getName(true)) + "\t"
                    + s.getSymbolType() + "\t" + s.getSource() + "\t" + (s.isPrimary() ? 1 : 0) + "\n");
                nsym++;
            }
        }
        note("symbols " + nsym);

        // ---- references --------------------------------------------------------------------
        int nref = 0;
        ReferenceManager rm = currentProgram.getReferenceManager();
        try (BufferedWriter w = open("refs.tsv")) {
            w.write("from\tto\ttype\tfrom_function\n");
            AddressIterator src = rm.getReferenceSourceIterator(currentProgram.getMemory(), true);
            while (src.hasNext()) {
                Address from = src.next();
                if (!real(from)) continue;
                Function in = fm.getFunctionContaining(from);
                String fe = in == null ? "" : hex(in.getEntryPoint());
                for (Reference r : rm.getReferencesFrom(from)) {
                    Address to = r.getToAddress();
                    if (!real(to)) continue;   // stack, register, constant, non-loaded sections
                    w.write(hex(from) + "\t" + hex(to) + "\t" + r.getReferenceType() + "\t" + fe + "\n");
                    nref++;
                }
            }
        }
        note("references " + nref);

        // ---- strings -----------------------------------------------------------------------
        int nstr = 0;
        try (BufferedWriter w = open("strings.tsv")) {
            w.write("address\tlength\tvalue\n");
            DataIterator di = currentProgram.getListing().getDefinedData(true);
            while (di.hasNext()) {
                Data d = di.next();
                if (!d.hasStringValue() || !real(d.getAddress())) continue;
                Object v = d.getValue();
                if (v == null) continue;
                w.write(hex(d.getAddress()) + "\t" + d.getLength() + "\t" + esc(v.toString()) + "\n");
                nstr++;
            }
        }
        note("strings " + nstr);
        note(String.format("inventories in %ds", (System.currentTimeMillis() - t0) / 1000));
        if (!decomp) return;

        // ---- decompile every function, in address order -----------------------------------
        //
        // In chunks, so the output is written in order and only one chunk of C is ever held: the
        // whole of `main` is ~500 MB of text, and DecompMany's notes record what holding all of it
        // at once does to a heap.
        long td = System.currentTimeMillis();
        final AtomicInteger done = new AtomicInteger(), failed = new AtomicInteger();
        final int total = fns.size(), CHUNK = 4000;
        try (BufferedWriter w = open("decomp.c")) {
            for (int start = 0; start < total; start += CHUNK) {
                List<Function> chunk = fns.subList(start, Math.min(total, start + CHUNK));
                Map<Address, String> byAddr = new ConcurrentHashMap<>();
                DecompilerCallback<Void> cb = new DecompilerCallback<Void>(currentProgram,
                        new DecompileConfigurer() {
                            @Override
                            public void configure(DecompInterface di) {
                                di.setOptions(new DecompileOptions());
                                di.toggleCCode(true);
                                di.toggleSyntaxTree(true);
                                di.setSimplificationStyle("decompile");
                            }
                        }) {
                    @Override
                    public Void process(DecompileResults res, ghidra.util.task.TaskMonitor m) {
                        if (res == null) return null;
                        Function f = res.getFunction();
                        if (res.decompileCompleted() && res.getDecompiledFunction() != null) {
                            byAddr.put(f.getEntryPoint(), res.getDecompiledFunction().getC());
                        } else {
                            failed.incrementAndGet();
                            byAddr.put(f.getEntryPoint(), "/* decompilation failed: "
                                + (res.getErrorMessage() == null ? "no result" : res.getErrorMessage().trim())
                                + " */\n");
                        }
                        done.incrementAndGet();
                        return null;
                    }
                };
                ParallelDecompiler.decompileFunctions(cb, chunk, monitor);
                cb.dispose();
                for (Function f : chunk) {
                    String body = byAddr.remove(f.getEntryPoint());
                    w.write("/* ==== " + hex(f.getEntryPoint()) + "  " + f.getSymbol().getName(true) + " ==== */\n");
                    w.write(body == null ? "/* the pool returned nothing for this one */\n" : body);
                    w.write("\n");
                }
                w.flush();
                long el = (System.currentTimeMillis() - td) / 1000;
                Files.write(dir.resolve("decomp.progress"), String.format("%d/%d decompiled, %d failed, %ds%n",
                    done.get(), total, failed.get(), el).getBytes(StandardCharsets.UTF_8));
            }
        }
        note(String.format("decompiled %d, failed %d, in %ds", done.get(), failed.get(),
            (System.currentTimeMillis() - td) / 1000));
    }
}
