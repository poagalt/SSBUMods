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
case "$op" in
  size) curl -sI "${A[@]}" --max-time 45 "$B$path" 2>/dev/null \
          | tr -d '\r' | awk 'tolower($1)=="content-length:"{print $2}' ;;
  list) curl -s "${A[@]}" --max-time 90 "$B${path%/}/" 2>/dev/null | tr -d '\r' ;;
  get)  curl -s "${A[@]}" --max-time 120 "$B$path" ;;
  del)  d=$(dirname "$path"); curl -s "${A[@]}" --max-time 60 -Q "-DELE $path" "$B${d%/}/" ;;
  *) echo "unknown op $op" >&2; exit 2 ;;
esac
