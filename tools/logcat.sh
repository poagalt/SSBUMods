#!/usr/bin/env bash
# Streams the plugin's log off the console WHILE THE GAME IS RUNNING, over Skyline's TCP logger.
#
# THIS IS THE ONE THAT BREAKS THE TEST LOOP OPEN. Until now every log came through
# tools/pull-crash.sh over FTP, and sys-ftpd exits the moment a game launches -- so reading a log
# meant: crash or quit Smash, go to hbmenu, relaunch ftpd, pull, relaunch Smash, get back into a
# match, reproduce. Minutes of the user's time per look, and it made "just add a log line and see"
# an expensive move instead of a free one.
#
# Skyline mirrors every `println!` to a TCP listener on port 6969. `src/telemetry/sink.rs` already
# prints every line it writes to the SD card -- see its `println!("{line}")` -- so the whole
# existing telemetry stream is ALREADY available live and nothing in the plugin has to change.
# Confirmed on 2026-09-18: 6969 accepts a connection while Smash is running and FTP is refusing.
#
# The listener is push-only and silent until something logs, so an idle menu produces nothing. That
# is not a failure; it means the probe has not fired yet.
#
#   bash tools/logcat.sh                    follow until interrupted, tee to work/logcat.txt
#   bash tools/logcat.sh 90                 follow for 90 seconds, then exit
#   bash tools/logcat.sh 90 ARTICLE         only lines containing ARTICLE
#
# Pair it with a playtest: start this, have the move performed, read work/logcat.txt. No quitting
# the game, no FTP, no reboot.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
. "$ROOT/tools/console.sh"
HOST="$CONSOLE_HOST"
PORT="${SKYLINE_LOG_PORT:-6969}"
SECS="${1:-0}"
FILTER="${2:-}"
OUT="$ROOT/work/logcat.txt"

PY=""
for c in py python3 python; do command -v "$c" >/dev/null 2>&1 && { PY="$c"; break; }; done
[ -n "$PY" ] || { echo "no python on PATH" >&2; exit 1; }

echo "== skyline log $HOST:$PORT -> ${OUT#"$ROOT/"} =="
[ "$SECS" != "0" ] && echo "   following for ${SECS}s" || echo "   following until interrupted"
[ -n "$FILTER" ] && echo "   filter: $FILTER"

SWITCH_HOST="$HOST" SKYLINE_LOG_PORT="$PORT" LOGCAT_SECS="$SECS" \
LOGCAT_FILTER="$FILTER" LOGCAT_OUT="$OUT" "$PY" - <<'PY'
import os, socket, sys, time

host = os.environ["SWITCH_HOST"]
port = int(os.environ["SKYLINE_LOG_PORT"])
secs = float(os.environ["LOGCAT_SECS"])
filt = os.environ["LOGCAT_FILTER"]
out = os.environ["LOGCAT_OUT"]

try:
    sock = socket.create_connection((host, port), timeout=8)
except OSError as e:
    print("  !! cannot reach %s:%d (%s)" % (host, port, e.__class__.__name__), file=sys.stderr)
    print("     Skyline only listens while the game is running; if Smash is closed there is",
          file=sys.stderr)
    print("     nothing to connect to and tools/pull-crash.sh over FTP is the right tool.",
          file=sys.stderr)
    raise SystemExit(1)

# Short recv timeout so the deadline is honoured even while the console is silent, which it is
# whenever no ported move is running.
sock.settimeout(1.0)
deadline = time.time() + secs if secs > 0 else None
buf, lines = b"", 0
with open(out, "w", encoding="utf-8") as fh:
    try:
        while deadline is None or time.time() < deadline:
            try:
                chunk = sock.recv(8192)
            except socket.timeout:
                continue
            except OSError:
                break
            if not chunk:
                print("  (console closed the connection)")
                break
            buf += chunk
            while b"\n" in buf:
                raw, buf = buf.split(b"\n", 1)
                line = raw.decode("utf-8", "replace").rstrip("\r")
                if filt and filt not in line:
                    continue
                lines += 1
                print(line, flush=True)
                fh.write(line + "\n")
                fh.flush()
    except KeyboardInterrupt:
        pass
sock.close()
print("  %d line(s) captured" % lines)
PY
