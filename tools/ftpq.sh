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
HOST=${FTP_HOST:-<console-ip>}
PORT=${FTP_PORT:-5000}
USER=${FTP_USER:-poaga}
PASS="$(tr -d '\r\n' < "$ROOT/.ftp-credentials.local" 2>/dev/null)"
B="ftp://$HOST:$PORT"
A=(-u "$USER:$PASS")
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
  get)  curl -s "${A[@]}" --max-time 120 "$B$path" ;;
  del)  d=$(dirname "$path"); curl -s "${A[@]}" --max-time 60 -Q "-DELE $path" "$B${d%/}/" ;;
  *) echo "unknown op $op" >&2; exit 2 ;;
esac
