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

# THE FTP SERVER'S SHAPE, worked out once and cached for ten minutes in work/.ftp-profile.
#
# Two servers have been on this console and they disagree on both things a URL needs:
#
#   sys-ftpd (to 2026-10-18)   a login (user poaga, password in .ftp-credentials.local), and the
#                              SD card IS the root: ftp://host:5000/atmosphere/...
#   sphaira's FTP (from 10-18) anonymous, and the root lists DEVICES (sdmc:, album_nand:, games:,
#                              install:), so the card is ftp://host:5000/sdmc:/atmosphere/...
#
# So every tool builds its URLs from FTP_BASE (= ftp://host:port + FTP_ROOT) and logs in as
# FTP_USER/FTP_PASS, and any absolute path it hands the server in a raw command (DELE, RNFR, MKD)
# takes FTP_ROOT in front. The probe tries the login first, then anonymous, and reads the root
# listing for `sdmc:`. A failed probe is NOT cached and falls back to the sys-ftpd shape, so a
# console that is simply off still produces the tool's own "cannot reach" message.
FTP_PORT=${FTP_PORT:-5000}
_ftp_cache="$ROOT/work/.ftp-profile"
_ftp_pass="$(tr -d '\r\n' < "$ROOT/.ftp-credentials.local" 2>/dev/null)"
if [ -z "${FTP_USER:-}" ] || [ -z "${FTP_ROOT+set}" ]; then
    if [ -f "$_ftp_cache" ] \
       && [ "$(sed -n 1p "$_ftp_cache")" = "$CONSOLE_HOST:$FTP_PORT" ] \
       && [ $(( $(date +%s) - $(stat -c %Y "$_ftp_cache") )) -lt 600 ]; then
        FTP_USER="$(sed -n 2p "$_ftp_cache")"
        FTP_ROOT="$(sed -n 3p "$_ftp_cache")"
    else
        _ftp_list=""
        if [ -n "$_ftp_pass" ]; then
            _ftp_list="$(curl -s --connect-timeout 4 --max-time 8 -l -u "poaga:$_ftp_pass" \
                "ftp://$CONSOLE_HOST:$FTP_PORT/" 2>/dev/null)" && [ -n "$_ftp_list" ] && FTP_USER=poaga
        fi
        if [ -z "${FTP_USER:-}" ]; then
            _ftp_list="$(curl -s --connect-timeout 4 --max-time 8 -l -u "anonymous:" \
                "ftp://$CONSOLE_HOST:$FTP_PORT/" 2>/dev/null)" && [ -n "$_ftp_list" ] && FTP_USER=anonymous
        fi
        if [ -n "${FTP_USER:-}" ]; then
            if printf '%s\n' "$_ftp_list" | tr -d '\r' | grep -qx 'sdmc:'; then FTP_ROOT=/sdmc:; else FTP_ROOT=""; fi
            mkdir -p "$ROOT/work"
            printf '%s\n%s\n%s\n' "$CONSOLE_HOST:$FTP_PORT" "$FTP_USER" "$FTP_ROOT" > "$_ftp_cache"
        else
            FTP_USER=poaga
            FTP_ROOT=""
        fi
    fi
fi
if [ "$FTP_USER" = "anonymous" ]; then FTP_PASS=""; else FTP_PASS="${FTP_PASS:-$_ftp_pass}"; fi
FTP_BASE="ftp://$CONSOLE_HOST:$FTP_PORT$FTP_ROOT"
export FTP_PORT FTP_USER FTP_PASS FTP_ROOT FTP_BASE
unset _ftp_cache _ftp_pass _ftp_list
