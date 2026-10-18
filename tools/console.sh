# Which console every tool talks to. ONE place, because there are two.
#
# Sourced, never run:   . "$ROOT/tools/console.sh"
#
# Sets and exports CONSOLE_HOST, and FTP_HOST / SWITCH_HOST to match, since those are the names
# the older tools and their Python helpers read.
#
# WHERE THE ADDRESS COMES FROM, in order:
#   1. FTP_HOST or SWITCH_HOST already in the environment -- a per-command override.
#   2. `.console-host.local` at the repo root: one line, the address. Gitignored like the FTP
#      password, because it is a fact about this desk, not about the project.
#   3. Nothing -- and then it STOPS.
#
# THERE IS DELIBERATELY NO BUILT-IN DEFAULT. Every tool used to fall back to <console-ip>, the
# 13.0.5 development console, and that was right for as long as there was one console. Since the
# second one (<console-serial>, 13.0.4, <console-ip>) became the one being played, a tool run without
# an override talks to the WRONG CARD and still succeeds: a deploy lands on a console nobody is
# testing, a crash pull reports "no new crashes" from a card that never ran the build. The shared
# skip manifest already left console #2 a month stale that way. A missing setting must be loud.
if [ -n "${FTP_HOST:-}" ]; then
    CONSOLE_HOST="$FTP_HOST"
elif [ -n "${SWITCH_HOST:-}" ]; then
    CONSOLE_HOST="$SWITCH_HOST"
elif [ -f "$ROOT/.console-host.local" ]; then
    CONSOLE_HOST="$(tr -d ' \r\n' < "$ROOT/.console-host.local")"
else
    CONSOLE_HOST=""
fi
if [ -z "$CONSOLE_HOST" ]; then
    echo "  no console set: write its address to .console-host.local, or pass FTP_HOST=..." >&2
    exit 2
fi
export CONSOLE_HOST
export FTP_HOST="$CONSOLE_HOST"
export SWITCH_HOST="$CONSOLE_HOST"
