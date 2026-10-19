#!/usr/bin/env bash
# Ad-hoc FTP query against the console, using the same credentials tools/deploy.sh uses.
#
# EXISTS BECAUSE THE PASSWORD FILE IS NOT A SHELL SCRIPT. `.ftp-credentials.local` holds the
# password and nothing else, so sourcing it runs the password as a command and every curl then
# fails with `Access denied: 430` -- which reads exactly like the console being unreachable. This
# wraps it once so that mistake cannot be made again ad hoc.
#
#   bash tools/ftpq.sh size <path>     Content-Length, or empty if absent
#   bash tools/ftpq.sh list <dir>      directory listing (truncates under load -- prefer size)
#   bash tools/ftpq.sh get <path>      file contents to stdout
#   bash tools/ftpq.sh del <path>      delete
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
. "$ROOT/tools/console.sh"
HOST=$CONSOLE_HOST
# Login and root from tools/console.sh (sys-ftpd and sphaira differ; see there).
B="$FTP_BASE"
A=(-u "$FTP_USER:$FTP_PASS")
op=${1:?op}; path=${2:-/}

# RETRIED, BECAUSE A FLAKY ANSWER IS WORSE THAN NO ANSWER.
#
# sys-ftpd drops roughly one transfer in eighty and answers some queries with nothing at all. A
# `size` that comes back empty reads as "the file is absent" and a `list` that comes back empty
# reads as "the directory is empty" -- both indistinguishable from the truth, and both have now
# caused real damage: tools/analyze.sh reported "no crash reports on the card" while two new ones
# sat there, and then moved a freshly-pulled telemetry log aside as stale because the size query
# it double-checked against came back blank.
#
# So an empty answer is retried before it is believed. Three attempts with a short pause is enough
# in practice; a file that is genuinely absent costs two extra round trips, which is nothing next
# to a diagnosis built on a phantom.
retry() {
    local i out
    for i in 1 2 3; do
        out=$("$@" 2>/dev/null)
        if [ -n "$out" ]; then
            printf '%s\n' "$out"
            return 0
        fi
        sleep 1
    done
    return 1
}

do_size() {
    curl -sI "${A[@]}" --max-time 45 "$B$path" 2>/dev/null \
      | tr -d '\r' | awk 'tolower($1)=="content-length:"{print $2}'
}
do_list() { curl -s "${A[@]}" --max-time 90 "$B${path%/}/" 2>/dev/null | tr -d '\r'; }

case "$op" in
  size) retry do_size ;;
  list) retry do_list ;;
  # `get` is not retried here: the caller wants the bytes on stdout and a partial body cannot be
  # detected from inside this function. Callers check the length themselves -- analyze.sh compares
  # against the size query and drops any crash report under a kilobyte.
  # 300s, and a retry, because the telemetry log is now MEGABYTES. At FLUSH_FRAMES=8 with a
  # per-frame probe line a real session is 5 MB, and a 120-second ceiling on that over sys-ftpd
  # is a truncation waiting to happen -- while the game is running it is worse, because the file
  # is being appended to as it is read. One retry covers the wedge described in tools/deploy.sh.
  get)  # NOT THROUGH COMMAND SUBSTITUTION. `out=$(curl ...)` strips every NUL byte, because bash
        # cannot hold one in a variable. The `printf x`/`${out%x}` pair preserved TRAILING
        # NEWLINES and so looked like it had made the capture faithful -- it had not. This was
        # correct for text and silently truncated every binary: librat_of_doom.nro came back
        # 174096 bytes of 245760, libarc_network.nro 61123 of 86016, libnro_hook.nro 36539 of
        # 53248. Roughly a third of an .nro is zeros.
        #
        # WHAT MAKES IT DANGEROUS is that it is not flaky. Five retries returned the same short
        # length every time, so it does not read as a transfer fault -- it reads as the file's
        # real size, and a hash comparison against a good copy then "proves" the console has a
        # different build. Both of those conclusions were drawn here before the cause was found.
        #
        # Every tool that pulls a binary (pull-exedump.sh, pull-crash.sh, deploy.sh) already used
        # `curl -o` directly and was never affected; nothing in vendor/ is contaminated.
        for a in 1 2; do
          t=$(mktemp)
          if curl -s "${A[@]}" --max-time 300 -o "$t" "$B$path" && [ -s "$t" ]; then
            cat "$t"; rm -f "$t"; break
          fi
          rm -f "$t"; sleep 2
        done ;;
  del)  d=$(dirname "$path"); curl -s "${A[@]}" --max-time 60 -Q "-DELE $FTP_ROOT$path" "$B${d%/}/" ;;
  *) echo "unknown op $op" >&2; exit 2 ;;
esac
