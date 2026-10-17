#!/usr/bin/env bash
# Grabs frames off the console's live video stream, so a move can be LOOKED AT instead of described.
#
# WHAT THIS CLOSES. Every article bug in this project has been reported as a shape -- "in the wrong
# hand", "straight down through the model", "stuck inside his body facing up" -- and each one cost a
# round trip through a person turning three dimensions into a sentence. tools/logcat.sh made the
# numbers live; this makes the picture live.
#
# REQUIRES SysDVR, WHICH IS INSTALLED NON-PERSISTENTLY AND ON PURPOSE. SysDVR 6.3 is the newest
# release and predates HOS 23.0.0 by seven months; its notes document support only to firmware 21.0,
# and this console runs 23.0.0 on an Atmosphere 1.12.0 built from the unreleased `23_support`
# branch. So `flags/boot2.flag` was deliberately NOT copied: the sysmodule is not in the boot path
# and cannot brick a boot. It has to be started by hand each session --
#
#   hbmenu -> SysDVR-conf -> start SysDVR   (then launch Smash; the stream survives it)
#
# -- and if it ever misbehaves, a reboot is a clean slate.
#
# MODE IS RTSP because `config/sysdvr/rtsp` is the shipped marker, and RTSP is the one mode ffmpeg
# can read with no SysDVR client at all: the whole pipeline is ffmpeg against a URL.
#
#   bash tools/grab.sh                 one frame, now
#   bash tools/grab.sh 12              12 frames at 6 fps (2 seconds of motion)
#   bash tools/grab.sh 12 3            12 frames at 3 fps
#
# Frames land in work/frames/ as PNGs, newest run replacing the last.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HOST="${SWITCH_HOST:-<console-ip>}"
PORT="${SYSDVR_RTSP_PORT:-6666}"
N="${1:-1}"
FPS="${2:-6}"
OUT="$ROOT/work/frames"
URL="rtsp://$HOST:$PORT/"

command -v ffmpeg >/dev/null 2>&1 || { echo "no ffmpeg on PATH" >&2; exit 1; }

# A closed port here almost always means SysDVR was never started this session, which is expected
# given it is deliberately not in the boot path -- so say that rather than printing an ffmpeg error.
if ! timeout 5 bash -c "echo > /dev/tcp/$HOST/$PORT" 2>/dev/null; then
    echo "  !! nothing listening on $HOST:$PORT" >&2
    echo "     SysDVR is installed WITHOUT boot2.flag, so it does not start by itself." >&2
    echo "     On the console: hbmenu -> SysDVR-conf -> start SysDVR, then launch the game." >&2
    exit 1
fi

rm -rf "$OUT"; mkdir -p "$OUT"
echo "== $URL -> ${OUT#"$ROOT/"} ($N frame(s) at ${FPS}fps) =="

# -rtsp_transport tcp: UDP drops frames across wifi and a torn frame is worse than a late one.
# -an: audio is never the question here.
ffmpeg -loglevel error -rtsp_transport tcp -i "$URL" \
       -an -vf "fps=$FPS" -frames:v "$N" "$OUT/frame_%03d.png" </dev/null

got=$(ls "$OUT"/frame_*.png 2>/dev/null | wc -l)
if [ "$got" = "0" ]; then
    echo "  !! connected but captured nothing -- is the console showing a game, or asleep?" >&2
    exit 1
fi
echo "  $got frame(s):"
ls -1 "$OUT"/frame_*.png | while read -r f; do printf '    %-28s %s bytes\n' "$(basename "$f")" "$(stat -c%s "$f")"; done
