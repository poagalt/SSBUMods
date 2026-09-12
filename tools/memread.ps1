# Pulls main's mapped regions out of the running game over TCP.
#
# The counterpart to plugin/src/telemetry/memserve.rs. Read that file's header for why this is a
# socket and not an SD-card write: the dump was abandoned three crashes ago because Skyline's fs
# layer moves ~300 KB/s and the game streams its audio off the same card. The bytes were never the
# problem, the sink was.
#
# WHY POWERSHELL AND NOT A RUST CLIENT. Every other tool here is a small Rust binary, and a fourth
# one would have been consistent. But this needs a TCP client, a byte-exact read loop and a file
# append, all of which the .NET base library already has on this machine with nothing to build --
# and a dump session needs the client to WORK the first time, because the alternative is asking for
# another game restart.
#
#   pwsh tools/memread.ps1                        # data + bss, the regions vendor/main lacks
#   pwsh tools/memread.ps1 -Regions text,rodata   # re-pull ones we already have
#   pwsh tools/memread.ps1 -Regions all
#
# REQUIRES SMASH RUNNING, AND RUNNING A BUILD THAT HAS THE SERVER IN IT. A Skyline plugin is loaded
# when the GAME LAUNCHES, so deploying to a console that is already running Smash changes nothing.
# That mistake cost an entire recording session once. The manifest this prints carries the plugin's
# build stamp for exactly that reason -- check it against tools/gen-buildstamp.sh output before
# believing a dump.
param(
    [string]$Target = "<console-ip>",
    [int]$Port = 6900,
    [string[]]$Regions = @("data", "bss"),
    [string]$OutDir = "vendor/main",
    [int]$ChunkKB = 256
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$outPath = Join-Path $root $OutDir
if (-not (Test-Path $outPath)) { New-Item -ItemType Directory -Force $outPath | Out-Null }

# Reads exactly $count bytes or throws. A partial read is not an error on a socket -- Read returns
# whatever has arrived -- so every read in this script goes through here.
function Read-Exact {
    param($stream, [int]$count)
    $buf = New-Object byte[] $count
    $off = 0
    while ($off -lt $count) {
        $n = $stream.Read($buf, $off, $count - $off)
        if ($n -le 0) { throw "peer closed after $off of $count bytes" }
        $off += $n
    }
    return $buf
}

function Send-Line {
    param($stream, [string]$line)
    $bytes = [System.Text.Encoding]::ASCII.GetBytes($line + "`n")
    $stream.Write($bytes, 0, $bytes.Length)
    $stream.Flush()
}

Write-Host "connecting to ${Target}:${Port}"
$client = New-Object System.Net.Sockets.TcpClient
$client.ReceiveTimeout = 30000
$client.SendTimeout = 30000
try {
    $client.Connect($Target, $Port)
} catch {
    Write-Host ""
    Write-Host "FAILED to connect. In order of likelihood:"
    Write-Host "  * Smash is not running, or was not RESTARTED after the deploy that added the"
    Write-Host "    server -- a Skyline plugin loads at game launch and nothing else."
    Write-Host "  * telemetry/memserve.rs has ENABLED = false."
    Write-Host "  * nn::socket did not resolve; the log line '[rat] MEMSERVE off' says so."
    Write-Host "  * the console is not at $Target."
    throw
}
$stream = $client.GetStream()

# The manifest first, always. It is text, ends with '# end', and carries the bases -- which matter
# as much as the bytes, since every Ghidra address and every crash-report offset is relative to one
# of them and they move with each game patch.
Send-Line $stream "map"
#
# READS THROUGH THE TRAILING NEWLINE, and that detail cost a run. Stopping at '# end' leaves the
# "`n" after it sitting in the stream, where the next read picks it up as the first byte of a
# 24-byte binary header -- which then fails as `bad reply magic "<newline>RATMEM0"`. A framing bug
# one byte wide reads exactly like a protocol mismatch.
$manifest = ""
while ($manifest -notmatch "# end`n") {
    $one = Read-Exact $stream 1
    $manifest += [System.Text.Encoding]::ASCII.GetString($one)
}
Write-Host ""
Write-Host $manifest.TrimEnd()

$plan = @{}
foreach ($line in $manifest -split "`n") {
    if ($line -match '^([a-z]+)\s+(0x[0-9a-f]+)\s+(0x[0-9a-f]+)') {
        # [uint64]"0x38f1604000" THROWS. PowerShell casts do not parse a hex prefix, so the
        # manifest's own notation has to go through Convert explicitly.
        $plan[$matches[1]] = @{
            Base = [Convert]::ToUInt64($matches[2].Substring(2), 16)
            Len  = [Convert]::ToUInt64($matches[3].Substring(2), 16)
        }
    }
}

if ($Regions.Count -eq 1 -and $Regions[0] -eq "all") {
    $Regions = @("text", "rodata", "data", "bss")
}

$manifestOut = Join-Path $outPath "rat-memserve.txt"
Set-Content -Path $manifestOut -Value $manifest -Encoding ascii

$chunk = [uint64]($ChunkKB * 1024)
foreach ($name in $Regions) {
    if (-not $plan.ContainsKey($name)) {
        Write-Host "skip ${name}: not in the manifest"
        continue
    }
    $base = $plan[$name].Base
    $len = $plan[$name].Len
    if ($len -eq 0) {
        # A zero length is the server refusing to guess. For .bss it means the MOD0 cross-check
        # failed, which is deliberate: the previous version of that code guessed 48 MB and read off
        # the end of the mapping.
        Write-Host "skip ${name}: server reported length 0 (MOD0 cross-check failed)"
        continue
    }

    # RESUMES, because a dropped transfer should not cost a restart.
    #
    # Every read is addressed rather than streamed, so resuming is just "start at the length of
    # what is already on disk". The first attempt at .data died 3.25 MB into 3.57 MB and had to be
    # thrown away, which is a silly thing to pay twice. Delete the .bin to force a clean pull.
    $file = Join-Path $outPath "rat-main-$name.bin"
    $off = [uint64]0
    if (Test-Path $file) {
        $have = (Get-Item $file).Length
        if ($have -ge $len) {
            Write-Host ("  $name already complete ({0:n0} bytes)" -f $have)
            continue
        }
        if ($have -gt 0) {
            $off = [uint64]$have
            Write-Host ("  $name resuming at {0:n0} of {1:n0} bytes" -f $have, $len)
        }
    }
    $fs = [System.IO.File]::Open($file, [System.IO.FileMode]::OpenOrCreate, [System.IO.FileAccess]::Write)
    $fs.Seek([long]$off, [System.IO.SeekOrigin]::Begin) | Out-Null
    $startOff = $off
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        while ($off -lt $len) {
            $want = $chunk
            if ($len - $off -lt $chunk) { $want = $len - $off }
            Send-Line $stream ("read 0x{0:x} 0x{1:x}" -f ($base + $off), $want)

            $hdr = Read-Exact $stream 24
            $magic = [System.Text.Encoding]::ASCII.GetString($hdr, 0, 8)
            if ($magic -ne "RATMEM01") { throw "bad reply magic '$magic' at offset $off" }
            $gotAddr = [System.BitConverter]::ToUInt64($hdr, 8)
            $gotLen = [System.BitConverter]::ToUInt64($hdr, 16)
            if ($gotLen -eq 0) { throw ("server refused 0x{0:x} (+0x{1:x}) -- out of its bounds" -f ($base + $off), $want) }
            if ($gotAddr -ne ($base + $off)) { throw ("reply address 0x{0:x} is not the 0x{1:x} requested" -f $gotAddr, ($base + $off)) }

            $body = Read-Exact $stream ([int]$gotLen)
            $fs.Write($body, 0, $body.Length)
            $off += $gotLen

            $pct = [int](100 * $off / $len)
            # Rate over THIS RUN's bytes, not $off -- after a resume $off starts in the millions
            # and would report a throughput the link never achieved.
            $kbs = 0
            if ($sw.Elapsed.TotalSeconds -gt 0) { $kbs = [int]((($off - $startOff) / 1024) / $sw.Elapsed.TotalSeconds) }
            Write-Host -NoNewline ("`r  $name  $pct%  {0:n0} of {1:n0} KB  ${kbs} KB/s     " -f ($off / 1024), ($len / 1024))
        }
    } finally {
        $fs.Close()
        Write-Host ""
    }
    Write-Host ("  $name -> $file  ({0:n0} bytes in {1:n1}s)" -f $off, $sw.Elapsed.TotalSeconds)
}

Send-Line $stream "bye"
$client.Close()
Write-Host ""
Write-Host "manifest -> $manifestOut"
