// A block-caching client for the arc_network protocol, because the stock one is latency-bound.
//
// THE MEASUREMENT THAT MOTIVATED THIS. Pulling 40,264 fighter files ran at 2.5 files/sec -- about
// 110 KB/s, an estimated 4.25 hours. Ping to the console is 5 ms, and 1/2.5 sec per file works out
// to roughly 80 round trips PER FILE. So the cost is the number of requests, not the link: a
// faster cable or a different FTP client would not have moved it.
//
// Where the 80 come from: smash_arc's read_file_data calls get_file_reader() fresh for every file,
// seeks (which throws away whatever the BufReader had), then streams the zstd decode -- and the
// decoder pulls in small chunks, each of which becomes one synchronous request/response over TCP.
// The protocol (vendor/network_reader_lib.rs) is strictly one opcode, one operation, one reply.
//
// WHAT THIS DOES INSTEAD. Reads are served out of a cache of large aligned blocks. A miss costs
// one seek and one read of BLOCK bytes; everything inside that block is then free. Seeks are
// bookkeeping on a local cursor and cost nothing at all, which is what kills the per-file seek.
// With 512 KiB blocks a 33 KB file is a fraction of one round trip rather than 80, and because
// arc-bulk sorts its work by offset in the archive, consecutive files usually land in a block
// that is already resident.
//
// This is not a substitute for having data.arc locally -- that removes the network entirely and is
// the right end state -- but it needs nothing from the console beyond what is already running.
use std::collections::HashMap;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::net::{TcpStream, ToSocketAddrs};

const OPERATION_READ: u8 = 0xFF;
const OPERATION_SEEK: u8 = 0xFE;
const SEEK_FROM_START: u8 = 0;
const SEEK_FROM_END: u8 = 1;
const SEEK_FROM_CURRENT: u8 = 2;
const RESULT_OK: u8 = 0;

/// Block size. Big enough that one request amortises across many small files, small enough that a
/// miss on a lone read is not itself expensive at ~110 KB/s of raw throughput.
const BLOCK: u64 = 512 * 1024;
/// Resident blocks. 96 x 512 KiB = 48 MiB, which is nothing on the host and covers a wide window
/// of the archive.
const MAX_BLOCKS: usize = 96;

pub struct BlockReader {
    sock: TcpStream,
    /// Our virtual cursor. The server's own position is only ever set immediately before a block
    /// fetch, so it does not have to track this.
    pos: u64,
    len: Option<u64>,
    blocks: HashMap<u64, Vec<u8>>,
    /// Insertion order, for eviction. Plain FIFO: the access pattern here is a moving window, so
    /// recency and insertion order agree closely enough that LRU bookkeeping is not worth it.
    order: Vec<u64>,
    pub requests: u64,
}

impl BlockReader {
    pub fn new<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let sock = TcpStream::connect(addr)?;
        sock.set_nodelay(true)?;
        Ok(Self {
            sock,
            pos: 0,
            len: None,
            blocks: HashMap::new(),
            order: Vec::new(),
            requests: 0,
        })
    }

    /// One protocol seek. Only used to position the server before a block read.
    fn remote_seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let (kind, off) = match pos {
            SeekFrom::Start(o) => (SEEK_FROM_START, o as i64),
            SeekFrom::End(o) => (SEEK_FROM_END, o),
            SeekFrom::Current(o) => (SEEK_FROM_CURRENT, o),
        };
        self.sock.write_all(&[OPERATION_SEEK])?;
        self.sock.write_all(&[kind])?;
        self.sock.write_all(&off.to_be_bytes())?;
        self.sock.flush()?;

        let mut result = [0u8];
        self.sock.read_exact(&mut result)?;
        if result != [RESULT_OK] {
            return Err(io::Error::new(io::ErrorKind::Other, "arc_network seek failed"));
        }
        let mut val = [0u8; 8];
        self.sock.read_exact(&mut val)?;
        Ok(u64::from_be_bytes(val))
    }

    /// One protocol read. The server always writes exactly `buf.len()` bytes -- zero-padding the
    /// tail past EOF -- and then an 8-byte count of how many were real, so the reply length is
    /// fixed and predictable.
    fn remote_read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.sock.write_all(&[OPERATION_READ])?;
        self.sock.write_all(&(buf.len() as u64).to_be_bytes())?;
        self.sock.flush()?;
        self.sock.read_exact(buf)?;
        let mut n = [0u8; 8];
        self.sock.read_exact(&mut n)?;
        Ok(u64::from_be_bytes(n) as usize)
    }

    fn fetch(&mut self, index: u64) -> io::Result<()> {
        if self.blocks.contains_key(&index) {
            return Ok(());
        }
        self.remote_seek(SeekFrom::Start(index * BLOCK))?;
        let mut buf = vec![0u8; BLOCK as usize];
        let got = self.remote_read(&mut buf)?;
        buf.truncate(got);
        self.requests += 1;

        if self.order.len() >= MAX_BLOCKS {
            let victim = self.order.remove(0);
            self.blocks.remove(&victim);
        }
        self.order.push(index);
        self.blocks.insert(index, buf);
        Ok(())
    }
}

impl Read for BlockReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        let index = self.pos / BLOCK;
        let within = (self.pos % BLOCK) as usize;
        self.fetch(index)?;
        let block = &self.blocks[&index];
        if within >= block.len() {
            // Past the real end of this block, which only happens at EOF.
            return Ok(0);
        }
        let n = out.len().min(block.len() - within);
        out[..n].copy_from_slice(&block[within..within + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for BlockReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.pos = match pos {
            SeekFrom::Start(o) => o,
            SeekFrom::Current(o) => (self.pos as i64 + o) as u64,
            SeekFrom::End(o) => {
                // The only case that has to ask the server, and it is asked once.
                let len = match self.len {
                    Some(l) => l,
                    None => {
                        let l = self.remote_seek(SeekFrom::End(0))?;
                        self.len = Some(l);
                        l
                    }
                };
                (len as i64 + o) as u64
            }
        };
        Ok(self.pos)
    }
}
