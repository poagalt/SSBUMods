// Decompiles named addresses out of a raw image of `main`'s .text, to C.
//
// WHY A HEADLESS SCRIPT AND NOT THE GUI. The question this answers is always the same shape:
// "what does the native code do when a catch connects, and how does it let go" -- and the answer
// is a handful of functions out of 39,635. A full auto-analysis of 60 MB of AArch64 takes hours
// and is not needed: Ghidra's decompiler works on one function at a time given a disassembly, so
// this disassembles from the entry point asked for, creates a function there, and prints the C.
//
// The addresses come from RAT_FUNCS (comma separated, module-relative hex), and the symbol table
// from RAT_SYMS in `<hex addr> <hex size> <name>` form -- so calls inside the decompiled body come
// out NAMED rather than as FUN_00xxxxxx, which is most of what makes the output readable.
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.SourceType;
import java.io.*;
import java.nio.file.*;
import java.util.*;

public class DecompAt extends GhidraScript {
    @Override
    public void run() throws Exception {
        String symPath = System.getenv("RAT_SYMS");
        String want = System.getenv("RAT_FUNCS");
        String outPath = System.getenv("RAT_OUT");
        if (want == null || want.isEmpty()) {
            println("RAT_FUNCS is empty; nothing to do");
            return;
        }

        // Names first. Every call in the decompiled body is resolved through these, and an
        // unnamed call is the difference between reading the logic and staring at it.
        int named = 0;
        int stubs = 0;
        if (symPath != null && Files.exists(Paths.get(symPath))) {
            for (String line : Files.readAllLines(Paths.get(symPath))) {
                if (line.startsWith("#")) continue;
                String[] f = line.trim().split("\s+");
                if (f.length < 3) continue;
                try {
                    long a = Long.parseLong(f[0], 16);
                    if (a == 0) continue;
                    Address addr = toAddr(a);
                    createLabel(addr, f[2], true, SourceType.USER_DEFINED);
                    named++;
                    // A LABEL IS NOT ENOUGH FOR THE DECOMPILER. It prints
                    // `func_0x00710052f1a0` for a call whose target has no FUNCTION at it,
                    // however many labels are there, and unnamed calls are most of what makes a
                    // status script unreadable. The imported PLT stubs are exactly the engine
                    // calls worth naming, so those get functions; the thousands of internal
                    // `def` symbols stay labels, because creating a function at each would mean
                    // disassembling the whole module.
                    if (f.length > 3 && f[3].equals("plt") && getFunctionAt(addr) == null) {
                        disassemble(addr);
                        createFunction(addr, f[2]);
                        stubs++;
                    }
                } catch (Exception e) { /* a bad row is not worth stopping for */ }
            }
        }
        println("applied " + named + " symbol(s), " + stubs + " call stub function(s)");

        DecompInterface di = new DecompInterface();
        DecompileOptions opts = new DecompileOptions();
        di.setOptions(opts);
        di.openProgram(currentProgram);

        StringBuilder out = new StringBuilder();
        for (String spec : want.split(",")) {
            spec = spec.trim();
            if (spec.isEmpty()) continue;
            long a = Long.parseLong(spec.replaceFirst("^0x", ""), 16);
            Address addr = toAddr(a);
            // Disassemble and create the function if this address has not been seen yet.
            Function fn = getFunctionAt(addr);
            if (fn == null) {
                disassemble(addr);
                fn = createFunction(addr, null);
            }
            if (fn == null) {
                out.append("/* " + spec + ": could not create a function here */\n\n");
                continue;
            }
            DecompileResults res = di.decompileFunction(fn, 180, monitor);
            out.append("/* ==== " + spec + "  " + fn.getName() + " ==== */\n");
            if (res != null && res.decompileCompleted()) {
                out.append(res.getDecompiledFunction().getC()).append("\n");
            } else {
                out.append("/* decompilation failed: "
                    + (res == null ? "no result" : res.getErrorMessage()) + " */\n\n");
            }
            println("decompiled " + spec + " (" + fn.getName() + ")");
        }
        di.dispose();

        if (outPath != null && !outPath.isEmpty()) {
            Files.write(Paths.get(outPath), out.toString().getBytes());
            println("wrote " + outPath);
        } else {
            println(out.toString());
        }
    }
}
