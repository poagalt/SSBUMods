// Decompile thousands of functions in ONE Ghidra invocation, on every core.
//
// WHY THIS EXISTS. `DecompAt.java` decompiles a comma-separated address list with a single
// `DecompInterface`, one function at a time. That is the right shape for "read me this function"
// and the wrong shape for the engine: `main` has roughly 99,000 functions, the measured rate is
// about 0.06s each, and a single-threaded pass is therefore ~13 hours. The machine has 24 cores.
//
// Ghidra ships `ghidra.app.decompiler.parallel.ParallelDecompiler`, which runs a pool of
// decompiler processes over a collection of functions inside one JVM with one import and one
// symbol-application pass. That last part matters as much as the threading: `DecompAt` re-applies
// 14,220 symbols on EVERY invocation, which is most of the ~7s per-batch startup that
// `decomp-all.sh` pays 60 times per module.
//
// THE ADDRESS LIST COMES FROM A FILE, not the command line. A comma-separated list of 99,000
// addresses is not a command line any shell will accept, and batching to work around that is the
// cost this script exists to remove.
//
//   RAT_SYMS   symbol table, same format DecompAt reads
//   RAT_LIST   file of addresses, one per line, `0x...` or bare hex
//   RAT_OUT    output file, same `/* ==== addr  name ==== */` delimiters the splitter expects
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.app.decompiler.parallel.*;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.SourceType;
import java.io.*;
import java.nio.file.*;
import java.util.*;
import java.util.concurrent.*;

public class DecompMany extends GhidraScript {
    @Override
    public void run() throws Exception {
        String symPath  = System.getenv("RAT_SYMS");
        String listPath = System.getenv("RAT_LIST");
        String outPath  = System.getenv("RAT_OUT");

        // ---- symbols, once for the whole run -------------------------------------------------
        int named = 0, stubs = 0;
        if (symPath != null && !symPath.isEmpty()) {
            for (String line : Files.readAllLines(Paths.get(symPath))) {
                String[] f = line.trim().split("\s+");
                if (f.length < 3) continue;
                try {
                    Address addr = toAddr(Long.parseLong(f[0], 16));
                    createLabel(addr, f[2], true, SourceType.USER_DEFINED);
                    named++;
                    if (f.length > 3 && f[3].equals("plt") && getFunctionAt(addr) == null) {
                        disassemble(addr);
                        if (createFunction(addr, f[2]) != null) stubs++;
                    }
                } catch (Exception e) { /* a bad row is not worth stopping for */ }
            }
        }
        println("applied " + named + " symbol(s), " + stubs + " call stub function(s)");

        // ---- the functions to do -------------------------------------------------------------
        //
        // Created serially, because `createFunction` mutates the program and the parallel
        // decompiler only READS it. This is the cheap half: disassembly is fast next to
        // decompilation.
        List<Function> fns = new ArrayList<>();
        List<String> specs = new ArrayList<>();
        int missing = 0;
        for (String line : Files.readAllLines(Paths.get(listPath))) {
            String spec = line.trim();
            if (spec.isEmpty() || spec.startsWith("#")) continue;
            // The list may be `addr name` or `addr name filename`; only the first field is used.
            spec = spec.split("\s+")[0];
            Address addr;
            try {
                addr = toAddr(Long.parseLong(spec.replaceFirst("^0x", ""), 16));
            } catch (Exception e) { continue; }
            Function fn = getFunctionAt(addr);
            if (fn == null) {
                disassemble(addr);
                fn = createFunction(addr, null);
            }
            if (fn == null) { missing++; continue; }
            fns.add(fn);
            specs.add(spec);
        }
        println("prepared " + fns.size() + " function(s), " + missing + " with no function to create");

        // ---- decompile them, on every core ---------------------------------------------------
        //
        // Results come back in an order the pool decides, so each is keyed by its entry point and
        // the output is reassembled in the caller's order afterwards. Writing straight through
        // would produce a file whose sections depend on thread scheduling, which is not something
        // a diff should have to cope with.
        Map<String, String> byAddr = new ConcurrentHashMap<>();
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
                String key = "0x" + f.getEntryPoint().toString();
                if (res.decompileCompleted() && res.getDecompiledFunction() != null) {
                    byAddr.put(key, res.getDecompiledFunction().getC());
                } else {
                    byAddr.put(key, "/* decompilation failed: "
                        + (res.getErrorMessage() == null ? "no result" : res.getErrorMessage())
                        + " */\n");
                }
                return null;
            }
        };

        long t0 = System.currentTimeMillis();
        ParallelDecompiler.decompileFunctions(cb, fns, monitor);
        cb.dispose();
        long secs = (System.currentTimeMillis() - t0) / 1000;
        println("decompiled " + byAddr.size() + " function(s) in " + secs + "s");

        // ---- one file, in the order the list asked for ---------------------------------------
        StringBuilder out = new StringBuilder();
        for (int i = 0; i < fns.size(); i++) {
            Function fn = fns.get(i);
            String key = "0x" + fn.getEntryPoint().toString();
            String body = byAddr.get(key);
            out.append("/* ==== ").append(specs.get(i)).append("  ")
               .append(fn.getName()).append(" ==== */\n");
            out.append(body == null ? "/* the pool returned nothing for this one */\n" : body)
               .append("\n");
        }
        Files.write(Paths.get(outPath), out.toString().getBytes());
        println("wrote " + outPath);
    }
}
