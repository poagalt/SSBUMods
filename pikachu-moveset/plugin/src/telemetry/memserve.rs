// Serves the game's own mapped memory to the PC over TCP, so that "what is actually in `main`"
// stops being a question this project has to guess at.
//
// WHY THIS EXISTS, AND WHY THE PREVIOUS ANSWER WAS WRONG THREE TIMES.
//
// `exedump.rs` did this job by writing the regions to the SD card. It crashed the game three
// separate times -- a training-mode load, a get-up attack, an f-smash -- always with the same
// signature: a null fault inside `nus3::audio update`, with no plugin frame anywhere in the
// backtrace. The diagnosis took two wrong turns before it was right:
//
//   1. "It is a synchronous write on the game thread." Moved it to a background thread. Still
//      crashed. That was the right diagnosis of the wrong quantity.
//   2. "The chunks are too big / it needs to rest between them." Added a 1 MB chunk and a 40 ms
//      rest. Still crashed, and the card showed exactly 4 MB of 37 MB written.
//
// The actual quantity is TOTAL TIME HOLDING THE CARD. Skyline's fs layer moves about 300 KB/s, so
// 37 MB is roughly two minutes of card occupancy no matter how it is sliced -- and the game
// streams its audio off that same card continuously. Chunking changes who waits, not how long.
// There is no chunk size and no rest interval that makes a multi-megabyte SD write safe during
// play, which is why `exedump::ENABLED` is false and stays false.
//
// A SOCKET IS NOT THE SAME RESOURCE. The regions do not have to go through the card at all. This
// module streams them out over TCP, where the only thing competing is the wifi radio, which the
// game is not using mid-match. The whole reason the dump was abandoned was a constraint that only
// ever applied to the sink, and the sink was replaceable.
//
// WHAT IS STILL MISSING WITHOUT IT. `main`'s `.text` and `.rodata` were dumped successfully before
// the crashes and are in `vendor/main/`. `.data` and `.bss` are not, and that is the gap that
// matters: VTABLES LIVE IN `.data`. `CaptureModule::Capture` is a virtual method reached through
// `vt[0x78]`, so `disasm --xrefs` on it returns `0 call site(s)` -- not because the caller is
// absent but because the call is indirect. Resolving it needs the vtable, and the vtable needs
// `.data`. Saying that behaviour "isn't in the code" was simply wrong; the honest statement is
// that the code is there and one specific edge of the call graph is unreachable to a static tool
// without this region.
//
// ---------------------------------------------------------------------------------------------
// EVERY SYMBOL IS RESOLVED AT RUNTIME, AND THAT IS A DELIBERATE COST.
//
// The obvious way to call `nn::socket::Socket` is an `extern "C"` block with its mangled name, the
// way `nnsdk` declares it. The failure mode is unacceptable here: if a name is not exported by any
// loaded NSO, the NRO relocation fails and THE WHOLE PLUGIN STOPS LOADING. Not this module -- the
// entire moveset, every move, silently back to vanilla Pikachu, and the symptom on the console is
// indistinguishable from a bad build.
//
// `nn::socket::*` is not in `main`'s dynamic symbol table -- 4,920 symbols in
// `vendor/main/rat-main-symbols.txt` and not one of them is `nn6socket`. It lives in the `sdk`
// NSO instead, which is almost certainly fine, but "almost certainly" is not a thing to bet the
// mod on. `nn::ro::LookupSymbol` asks the same question at runtime and answers with a null
// instead of a load failure, so a wrong guess here costs this module and nothing else.
//
// `Recv` and `Close` are additionally NOT DECLARED BY `nnsdk` AT ALL; their names below follow the
// same Itanium mangling as the ones that are, and are the only two in this file not cross-checked
// against an existing declaration. If either fails to resolve the server degrades to push-only
// (see `serve`) rather than going away.
//
// ---------------------------------------------------------------------------------------------
// READS ARE BOUNDED TO `main`'s OWN REGIONS, FOR NOW.
//
// An arbitrary-address read server is the instrument this project actually wants -- chasing a
// vtable pointer into the heap, reading a live `BattleObjectModuleAccessor`. It needs
// `svcQueryMemory` (SVC 0x06) to know whether an address is mapped before touching it, because an
// unmapped read is a fault.
//
// That is a SEPARATE STEP ON PURPOSE. An SVC the process is not permitted to make is a kernel
// fatal, not an error return -- there is no way to probe for it safely, and it would crash at
// plugin load, which is the one failure that wastes a whole playtest. So this version reads only
// inside the four region bases `getRegionAddress` reports, which are known-mapped by construction
// and cannot fault. Prove the transport first, then extend the reach.
use skyline::hooks::{getRegionAddress, Region};

/// Flipped on for a dump session. A listening socket is not something to leave in a shipped mod
/// indefinitely, but it costs nothing while idle and the alternative is another playtest to turn
/// it on.
pub const ENABLED: bool = true;

/// Nothing else on this console answers here. sys-ftpd is 5000, arc_network is 43022.
const PORT: u16 = 6900;

const AF_INET: i32 = 2;
const SOCK_STREAM: i32 = 1;
const SOL_SOCKET: i32 = 0xffff;
const SO_REUSEADDR: i32 = 0x0004;

/// Matches `exedump::BSS_MAX`. A `.bss` length is computed, not reported, so it is sanity-bounded.
const BSS_MAX: usize = 64 << 20;

#[repr(C)]
struct SockAddrIn {
    len: u8,
    family: u8,
    port: u16,
    addr: [u8; 4],
    zero: u64,
}

type FnSocket = extern "C" fn(i32, i32, i32) -> i32;
type FnBind = extern "C" fn(i32, *const SockAddrIn, u32) -> i32;
type FnListen = extern "C" fn(i32, i32) -> i32;
type FnAccept = extern "C" fn(i32, *mut SockAddrIn, *mut u32) -> i32;
type FnSend = extern "C" fn(i32, *const u8, u64, i32) -> i64;
type FnRecv = extern "C" fn(i32, *mut u8, u64, i32) -> i64;
type FnClose = extern "C" fn(i32) -> i32;
type FnSetSockOpt = extern "C" fn(i32, i32, i32, *const u8, u32) -> i32;
type FnInitialize = extern "C" fn(*mut u8, u64, u64, i32) -> u32;

/// Resolves one mangled name, or `None`.
///
/// The `Result` from `LookupSymbol` is checked AND the address is checked, because a zero address
/// with a success code would otherwise be transmuted into a function and called.
unsafe fn sym(mangled: &str) -> Option<usize> {
    let mut out: usize = 0;
    let name = format!("{mangled}\0");
    let rc = skyline::nn::ro::LookupSymbol(&mut out as *mut usize, name.as_ptr());
    if rc == 0 && out != 0 {
        Some(out)
    } else {
        None
    }
}

struct Sock {
    socket: FnSocket,
    bind: FnBind,
    listen: FnListen,
    accept: FnAccept,
    send: FnSend,
    setsockopt: FnSetSockOpt,
    initialize: Option<FnInitialize>,
    /// Absent means push-only; see `serve`.
    recv: Option<FnRecv>,
    close: Option<FnClose>,
}

unsafe fn resolve() -> Option<Sock> {
    // The six that `nnsdk` also declares, so these names are cross-checked rather than invented.
    let socket = sym("_ZN2nn6socket6SocketEiii")?;
    let bind = sym("_ZN2nn6socket4BindEiPK8sockaddrj")?;
    let listen = sym("_ZN2nn6socket6ListenEii")?;
    let accept = sym("_ZN2nn6socket6AcceptEiP8sockaddrPj")?;
    let send = sym("_ZN2nn6socket4SendEiPKvmi")?;
    let setsockopt = sym("_ZN2nn6socket10SetSockOptEiiiPKvj")?;
    Some(Sock {
        socket: core::mem::transmute(socket),
        bind: core::mem::transmute(bind),
        listen: core::mem::transmute(listen),
        accept: core::mem::transmute(accept),
        send: core::mem::transmute(send),
        setsockopt: core::mem::transmute(setsockopt),
        initialize: sym("_ZN2nn6socket10InitializeEPvmmi").map(|a| core::mem::transmute(a)),
        // The two this file invents.
        recv: sym("_ZN2nn6socket4RecvEiPvmi").map(|a| core::mem::transmute(a)),
        close: sym("_ZN2nn6socket5CloseEi").map(|a| core::mem::transmute(a)),
    })
}

/// Opens a listening socket, initialising the socket library only if it turns out to be needed.
///
/// ORDER MATTERS. Smash initialises `nn::socket` itself for online play, and calling
/// `nn::socket::Initialize` a second time on an already-initialised nn library is the kind of
/// misuse those libraries answer with an abort rather than an error code. So this ASKS FIRST:
/// `Socket()` succeeding proves the library is up, and `Initialize` is only reached when it does
/// not.
unsafe fn listener(s: &Sock) -> Option<i32> {
    let mut fd = (s.socket)(AF_INET, SOCK_STREAM, 0);
    if fd < 0 {
        if let Some(init) = s.initialize {
            const POOL: usize = 0x100000;
            let layout = std::alloc::Layout::from_size_align(POOL, 0x1000).ok()?;
            let pool = std::alloc::alloc_zeroed(layout);
            if pool.is_null() {
                return None;
            }
            let rc = init(pool, POOL as u64, 0x20000, 14);
            crate::telemetry::sink::diag(format!("[rat] MEMSERVE socket init rc={rc:#x}"));
        }
        fd = (s.socket)(AF_INET, SOCK_STREAM, 0);
        if fd < 0 {
            return None;
        }
    }

    let one: u32 = 1;
    (s.setsockopt)(
        fd,
        SOL_SOCKET,
        SO_REUSEADDR,
        &one as *const u32 as *const u8,
        4,
    );

    // Port is big-endian on the wire. Byte-swapped here rather than through `InetHtons`, which
    // would be one more symbol to resolve for one byte swap.
    let addr = SockAddrIn {
        len: core::mem::size_of::<SockAddrIn>() as u8,
        family: AF_INET as u8,
        port: PORT.to_be(),
        addr: [0, 0, 0, 0],
        zero: 0,
    };
    if (s.bind)(fd, &addr, core::mem::size_of::<SockAddrIn>() as u32) != 0 {
        if let Some(close) = s.close {
            close(fd);
        }
        return None;
    }
    if (s.listen)(fd, 1) != 0 {
        if let Some(close) = s.close {
            close(fd);
        }
        return None;
    }
    Some(fd)
}

/// `Send` until the whole slice is gone. Returns false only once the peer has really hung up.
///
/// RETRIES, BECAUSE THE FIRST VERSION DID NOT AND LOST A TRANSFER. It treated any `n <= 0` as a
/// closed connection and returned, which closes the socket and drops the client mid-region --
/// reported on the PC as `peer closed after 0 of 24 bytes`, i.e. as though the game had died.
/// `Send` can transiently fail with the socket buffer full at a megabyte a second, and that is not
/// the same event as a hangup. Bounded, so a genuinely dead peer still ends the loop.
unsafe fn send_all(s: &Sock, fd: i32, buf: &[u8]) -> bool {
    let mut sent = 0usize;
    let mut stalls = 0u32;
    while sent < buf.len() {
        let n = (s.send)(fd, buf[sent..].as_ptr(), (buf.len() - sent) as u64, 0);
        if n > 0 {
            sent += n as usize;
            stalls = 0;
            continue;
        }
        stalls += 1;
        if stalls > 200 {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    true
}

/// The four regions, with the lengths derived the way `exedump` derives them.
///
/// `getRegionAddress` hands over bases and no lengths, so `.text`, `.rodata` and `.data` take
/// their sizes from the gap up to the next base. `.bss` is last and has no next base, so its
/// length comes from MOD0 and is bounded -- an over-long `.bss` would read off the end of the
/// mapping, which is the one way this module could still fault.
unsafe fn regions() -> Vec<(&'static str, usize, usize)> {
    let text = getRegionAddress(Region::Text) as usize;
    let rodata = getRegionAddress(Region::Rodata) as usize;
    let data = getRegionAddress(Region::Data) as usize;
    // NOT `.bss`'s BASE, whatever the enum is called. See the note below.
    let module_end = getRegionAddress(Region::Bss) as usize;

    let mut out = vec![
        ("text", text, rodata.saturating_sub(text)),
        ("rodata", rodata, data.saturating_sub(rodata)),
    ];

    // MOD0 IS THE AUTHORITY ON WHERE `.bss` STARTS, and `Region::Bss` is not.
    //
    // The first run of this module described one 38 MB region called "data". A crash report
    // settled what that actually was:
    //
    //     Module 02: 0x5220204000-0x5223bcd000  cross2_Release.nss   <- main
    //     Module 03: 0x522764c000-0x52279b9000  multimedia
    //
    // 0x522764c000 is what `getRegionAddress(Region::Bss)` returned, and it is the base of the
    // NEXT MODULE -- i.e. the page-aligned END of main, not the start of its `.bss`. MOD0 puts
    // `.bss` at module-relative 0x52aa000..0x7447100, so the single "data" span was `.data`
    // (3.57 MB) and `.bss` (33.8 MB) glued together and labelled with the first one's name.
    //
    // BOTH HALVES ARE MAPPED, so the old span was never going to fault -- it was mislabelled, not
    // dangerous. But the label is the whole point of the dump: vtables are in `.data`, and being
    // told `.data` is 38 MB when it is 3.57 MB sends a reader looking for them in 34 MB of
    // zero-initialised runtime state.
    //
    // The earlier version cross-checked MOD0's bss_start against `Region::Bss` and returned 0 on a
    // mismatch, which is why the first manifest said `bss 0x0`. The check was right and the
    // conclusion was backwards: the disagreement WAS the finding, not a reason to give up.
    let (bss_start, bss_end) = mod0_bss(text);
    let split = text + bss_start;
    if bss_start != 0 && split > data && split < module_end && bss_end > bss_start {
        out.push(("data", data, split - data));
        // `.bss` IS SPARSE, AND READING IT LINEARLY FAULTS. Advertised with length 0 so a client
        // skips it by default.
        //
        // Paid for directly: a 35 MB `.bss` pull died 6.5 MB in, and unlike the two crashes before
        // it, that report named THIS thread and `__memcpy_device + 0x1d4` -- the `copy_from_slice`
        // below -- faulting at `.bss + 0x698000`, inside the chunk being read. The 35 MB is address
        // space RESERVED for `.bss`; only the pages the game has actually touched are backed, so a
        // sequential sweep is guaranteed to walk into a hole.
        //
        // It is left in the manifest by NAME so the region is documented rather than quietly
        // missing, and so `bss_end` stays visible. Serving it needs `svcQueryMemory` (SVC 0x06) to
        // test each page first, which is the deferred step described at the top of this file --
        // and `.bss` is zero-initialised runtime state, not code, so it is not what the dump was
        // ever for. Vtables are in `.data`, which is fully mapped and complete.
        let _ = bss_end;
        out.push(("bss_sparse_unreadable", split, 0));
    } else {
        // MOD0 was not the shape this assumes. Fall back to exactly what the region bases
        // describe, under a name that does not claim more than it knows.
        out.push(("data_and_bss", data, module_end.saturating_sub(data)));
    }
    out
}

/// main's `.bss` bounds as MODULE-RELATIVE offsets, out of MOD0. `(0, 0)` if the header is not the
/// shape this assumes.
///
/// MOD0 sits at `base+4` as an offset to itself, and carries `bss_start`/`bss_end` as SIGNED
/// offsets from the MOD0 header. Read off this console's own image they are:
///
///     MOD0 at +0x8:  "MOD0"  dyn=0x529e740  bss_start=0x52a9ff8  bss_end=0x74470f8
///     so .bss spans module-relative 0x52aa000 .. 0x7447100
///
/// which is checked against `.data`'s base and the module end by the caller rather than trusted.
/// An earlier version of this guessed 48 MB for `.bss` and read off the end of the mapping.
unsafe fn mod0_bss(text: usize) -> (usize, usize) {
    let mod0 = text + std::ptr::read_unaligned((text + 4) as *const u32) as usize;
    if std::ptr::read_unaligned(mod0 as *const u32) != u32::from_le_bytes(*b"MOD0") {
        return (0, 0);
    }
    let start = mod0 as i64 + std::ptr::read_unaligned((mod0 + 8) as *const i32) as i64;
    let end = mod0 as i64 + std::ptr::read_unaligned((mod0 + 12) as *const i32) as i64;
    if start <= text as i64 || end <= start || (end - start) as usize > BSS_MAX {
        return (0, 0);
    }
    (start as usize - text, end as usize - text)
}

/// 24-byte reply header, so a client never has to guess how much follows.
fn chunk_header(addr: usize, len: usize) -> Vec<u8> {
    let mut h = Vec::with_capacity(24);
    h.extend_from_slice(b"RATMEM01");
    h.extend_from_slice(&(addr as u64).to_le_bytes());
    h.extend_from_slice(&(len as u64).to_le_bytes());
    h
}

/// The manifest: bases and lengths, as text, so the dump is self-describing.
///
/// THE BASES ARE AS IMPORTANT AS THE BYTES. Every offset in a crash report and every address in
/// Ghidra is relative to one of these, and they move with every game patch.
unsafe fn manifest(plan: &[(&'static str, usize, usize)]) -> String {
    let mut m = String::from("# rat memserve; main regions\n");
    m.push_str(&format!("# build {}\n", crate::telemetry::buildstamp::BUILD));
    for (name, base, len) in plan {
        m.push_str(&format!("{name} {base:#x} {len:#x}\n"));
    }
    m.push_str("# end\n");
    m
}

/// Streams one region in pieces, each with its own header.
///
/// CHUNKED FOR THE PEER, NOT FOR THE GAME. The SD version chunked to try to give the card a rest,
/// which never worked. Here it is only so that a client can show progress and so that a dropped
/// connection is noticed part-way instead of after 30 MB.
unsafe fn send_region(s: &Sock, fd: i32, base: usize, len: usize) -> bool {
    const CHUNK: usize = 256 << 10;
    let mut buf = vec![0u8; CHUNK];
    let mut off = 0usize;
    while off < len {
        let n = CHUNK.min(len - off);
        if !send_all(s, fd, &chunk_header(base + off, n)) {
            return false;
        }
        // THE COPY IS THE WHOLE FIX. NEVER HAND THE GAME'S OWN MAPPED PAGES TO AN IPC CALL.
        //
        // This module used to pass `from_raw_parts(base + off, n)` straight to `Send`, pointing
        // into main's live image. That crashed the game twice, and the crash reports say why:
        //
        //     crash 1  str w8, [x9, #0x13c]   -> .data+0x32613c   transfer window 0 .. 0x340000
        //     crash 2  str w8, [x9, #0xc8]    -> .data+0x384928   transfer window 0x340000 .. 0x393000
        //
        // Both are ordinary stores to static, mapped, writable globals, both report
        // `Fault Address 0x0`, and both target an address INSIDE the range being transmitted at
        // that instant. A store to mapped memory cannot fault -- unless the pages are no longer
        // mapped. Horizon transfers a large IPC buffer by MAP-ALIAS: the kernel remaps the sender's
        // pages into the service's address space for the duration of the call, so for those
        // microseconds the game's own `.data` is missing from the game's own address space. The
        // `nus3::audio update` thread stored into a page that had been lent to bsd:s.
        //
        // THIS ALSO RETIRES THE SD-CARD EXPLANATION, which was wrong. `exedump.rs` blames three
        // crashes on Skyline's ~300 KB/s fs throughput and the game needing the card. Same
        // mechanism: `write_all` on a slice into `.text` lends those pages to the fs service. The
        // thread name in all five reports is the same (`nus3::audio update`) because the audio
        // streamer is simply the busiest toucher of static memory. Chunk size and rest intervals
        // were never going to help, which is exactly what was observed.
        buf[..n].copy_from_slice(std::slice::from_raw_parts((base + off) as *const u8, n));
        if !send_all(s, fd, &buf[..n]) {
            return false;
        }
        off += n;
        // A YIELD, NOT A REST. The SD dump's 40 ms rests were pointless because the card's total
        // occupancy was fixed either way; this is a different claim -- one sleep per 256 KB caps
        // this thread's share of the scheduler without meaningfully slowing a 3.5 MB region.
        // Whether the last crash was this thread's fault is genuinely unresolved, and 14 ms
        // against a 3-second transfer is a cheap way to stop asking.
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    true
}

/// Reads one newline-terminated request, or `None` on hangup.
unsafe fn read_line(fd: i32, recv: FnRecv) -> Option<String> {
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = recv(fd, byte.as_mut_ptr(), 1, 0);
        if n <= 0 {
            return None;
        }
        if byte[0] == b'\n' {
            return Some(String::from_utf8_lossy(&line).trim().to_string());
        }
        if line.len() < 256 {
            line.push(byte[0]);
        }
    }
}

/// Serves one accepted connection.
///
/// TWO MODES, and the fallback is the point. With `Recv` the client drives: `map`, then
/// `read <hex-addr> <hex-len>` for whatever it wants, which is what makes this an instrument
/// rather than a dump. Without `Recv` -- if that one invented mangled name is wrong -- the server
/// cannot be asked anything, so it pushes the manifest and all four regions and hangs up. That
/// still delivers `.data` and `.bss`, which is the thing actually being chased.
unsafe fn serve(s: &Sock, fd: i32) {
    let plan = regions();
    let m = manifest(&plan);

    let recv = match s.recv {
        Some(r) => r,
        None => {
            if send_all(s, fd, m.as_bytes()) {
                for (_, base, len) in &plan {
                    if *len > 0 && !send_region(s, fd, *base, *len) {
                        break;
                    }
                }
            }
            return;
        }
    };

    loop {
        let req = match read_line(fd, recv) {
            Some(r) => r,
            None => return,
        };
        let mut parts = req.split_whitespace();
        match parts.next() {
            Some("map") => {
                if !send_all(s, fd, m.as_bytes()) {
                    return;
                }
            }
            Some("all") => {
                for (_, base, len) in &plan {
                    if *len > 0 && !send_region(s, fd, *base, *len) {
                        return;
                    }
                }
            }
            Some("read") => {
                let addr = parts
                    .next()
                    .and_then(|a| usize::from_str_radix(a.trim_start_matches("0x"), 16).ok());
                let len = parts
                    .next()
                    .and_then(|a| usize::from_str_radix(a.trim_start_matches("0x"), 16).ok());
                match (addr, len) {
                    // BOUNDED, and this is the whole safety story of the module. A request is
                    // served only if it lies entirely inside one of the four regions whose base
                    // and length came from `getRegionAddress` and MOD0. Anything else gets a
                    // zero-length header, not a fault.
                    (Some(a), Some(l)) if in_plan(&plan, a, l) => {
                        if !send_all(s, fd, &chunk_header(a, l)) {
                            return;
                        }
                        // Copied first, for the reason spelled out in `send_region`: handing
                        // main's own mapped pages to `Send` unmaps them from under the game.
                        let mut buf = vec![0u8; l];
                        buf.copy_from_slice(std::slice::from_raw_parts(a as *const u8, l));
                        if !send_all(s, fd, &buf) {
                            return;
                        }
                    }
                    _ => {
                        if !send_all(s, fd, &chunk_header(addr.unwrap_or(0), 0)) {
                            return;
                        }
                    }
                }
            }
            Some("bye") | None => return,
            _ => {
                if !send_all(s, fd, b"# unknown request\n") {
                    return;
                }
            }
        }
    }
}

fn in_plan(plan: &[(&'static str, usize, usize)], addr: usize, len: usize) -> bool {
    // Capped at 1 MB because the `read` path now COPIES into a heap buffer of this size before
    // sending, and the client chunks at 256 KB anyway. A 32 MB request would be a 32 MB allocation
    // inside a game that is already tight on heap.
    if len == 0 || len > (1 << 20) {
        return false;
    }
    plan.iter().any(|(_, base, rlen)| {
        *rlen > 0 && addr >= *base && addr.checked_add(len).map_or(false, |e| e <= base + rlen)
    })
}

/// Spawns the server thread. Called once from `telemetry::install`.
pub fn install() {
    if !ENABLED {
        return;
    }
    std::thread::spawn(|| unsafe {
        let s = match resolve() {
            Some(s) => s,
            None => {
                crate::telemetry::sink::diag(
                    "[rat] MEMSERVE off: nn::socket did not resolve; nothing else is affected"
                        .to_string(),
                );
                return;
            }
        };
        crate::telemetry::sink::diag(format!(
            "[rat] MEMSERVE resolved; recv={} close={}",
            s.recv.is_some(),
            s.close.is_some()
        ));

        // RETRIED, BECAUSE THE NETWORK IS NOT UP AT PLUGIN LOAD. A plugin is loaded when the game
        // launches, long before anything has brought an interface up, so the first Bind can fail
        // for reasons that stop being true a few seconds later.
        let fd = loop {
            if let Some(fd) = listener(&s) {
                break fd;
            }
            std::thread::sleep(std::time::Duration::from_secs(5));
        };
        crate::telemetry::sink::diag(format!("[rat] MEMSERVE listening on {PORT}"));

        loop {
            let mut peer = SockAddrIn {
                len: 0,
                family: 0,
                port: 0,
                addr: [0; 4],
                zero: 0,
            };
            let mut plen = core::mem::size_of::<SockAddrIn>() as u32;
            let c = (s.accept)(fd, &mut peer, &mut plen);
            if c < 0 {
                std::thread::sleep(std::time::Duration::from_millis(500));
                continue;
            }
            serve(&s, c);
            if let Some(close) = s.close {
                close(c);
            }
        }
    });
}
