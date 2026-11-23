#!/usr/bin/env bash
# Generates the audio fixtures and fixture packs used by the core tests.
# Needs ffmpeg (a developer tool, not an app dependency). The generated files
# are committed, so tests and CI do not need ffmpeg.
#
# Every sound is an original tone burst made here: a sine wave with a short
# fade in and out. Each pack group has its own length so a test can tell which
# group played from the sample's duration.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

command -v ffmpeg >/dev/null 2>&1 || { echo "ffmpeg is required" >&2; exit 1; }

audio=src-tauri/tests/fixtures/audio
packs=src-tauri/tests/fixtures/packs
rm -rf "$audio" "$packs"
mkdir -p "$audio" "$packs"

# tone <out> <freq Hz> <ms> <rate> <channels> [extra ffmpeg output args...]
tone() {
  local out=$1 freq=$2 ms=$3 rate=$4 ch=$5
  shift 5
  local secs
  secs=$(awk "BEGIN{printf \"%.3f\", $ms/1000}")
  local fade
  fade=$(awk "BEGIN{printf \"%.3f\", ($ms/1000)/4}")
  local start
  start=$(awk "BEGIN{printf \"%.3f\", $secs-$fade}")
  ffmpeg -v error -y -f lavfi -i "sine=frequency=$freq:duration=$secs:sample_rate=$rate" \
    -af "afade=t=in:d=$fade,afade=t=out:st=$start:d=$fade,volume=0.5" \
    -ac "$ch" -ar "$rate" "$@" "$out"
}

# Format x channels x rate matrix, 200 ms each: tone-<ch>-<rate>.<ext>
for rate in 44100 48000; do
  for ch in 1 2; do
    name=$([ "$ch" = 1 ] && echo mono || echo stereo)
    base="$audio/tone-$name-$rate"
    tone "$base.wav" 440 200 "$rate" "$ch" -c:a pcm_s16le
    tone "$base.flac" 440 200 "$rate" "$ch" -c:a flac
    tone "$base.mp3" 440 200 "$rate" "$ch" -c:a libmp3lame -b:a 96k
    # ffmpeg's built-in Vorbis encoder is stereo only (no libvorbis here), so
    # there is no mono OGG fixture. Mono Vorbis is not covered by a fixture.
    if [ "$ch" = 2 ]; then
      tone "$base.ogg" 440 200 "$rate" "$ch" -c:a vorbis -strict -2 -b:a 96k
    fi
  done
done

# A WAV that is named .ogg: the format must come from the content.
cp "$audio/tone-mono-44100.wav" "$audio/wav-named-ogg.ogg"

# A silent WAV (every value zero), 100 ms.
ffmpeg -v error -y -f lavfi -i "anullsrc=r=44100:cl=mono" -t 0.1 -c:a pcm_s16le "$audio/silent.wav"

# A 4 second WAV (over the 3 second limit). 8 bit, low rate, to keep it small.
tone "$audio/long-4s.wav" 300 4000 4000 1 -c:a pcm_u8

# Not audio at all.
printf 'This is not an audio file.\n' > "$audio/not-audio.txt"

# ---- Fixture packs ----
# Sounds for the pack groups, mono 44.1 kHz WAV. Lengths identify the group:
#   down: default 40/45/50 ms (three files), space 70, enter 90, backspace 110,
#         modifier 130
#   up:   default 150, space 170
mkdir -p "$packs"/{full,default-only,up-space-only,no-up,shared,extra-field,formats}/{down,up}
mk() { tone "$1" "$2" "$3" 44100 1 -c:a pcm_s16le; }

for p in full default-only up-space-only no-up shared extra-field; do
  mk "$packs/$p/down/a1.wav" 500 40
  mk "$packs/$p/down/a2.wav" 520 45
  mk "$packs/$p/down/a3.wav" 540 50
done
mk "$packs/full/down/space.wav" 300 70
mk "$packs/full/down/enter.wav" 350 90
mk "$packs/full/down/back.wav" 250 110
mk "$packs/full/down/mod.wav" 200 130
mk "$packs/full/up/a.wav" 800 150
mk "$packs/full/up/space.wav" 700 170
mk "$packs/up-space-only/up/space.wav" 700 170
mk "$packs/no-up/down/space.wav" 300 70   # present but unused by pack.json

cat > "$packs/full/pack.json" <<'JSON'
{
  "name": "Full test pack",
  "author": "Test Author",
  "license": "CC0-1.0",
  "keys": {
    "default": ["down/a1.wav", "down/a2.wav", "down/a3.wav"],
    "space": ["down/space.wav"],
    "enter": ["down/enter.wav"],
    "backspace": ["down/back.wav"],
    "modifier": ["down/mod.wav"]
  },
  "up": {
    "default": ["up/a.wav"],
    "space": ["up/space.wav"]
  }
}
JSON

cat > "$packs/default-only/pack.json" <<'JSON'
{
  "name": "Default only",
  "license": "CC0-1.0",
  "keys": { "default": ["down/a1.wav", "down/a2.wav", "down/a3.wav"] }
}
JSON

cat > "$packs/up-space-only/pack.json" <<'JSON'
{
  "name": "Up space only",
  "license": "CC0-1.0",
  "keys": { "default": ["down/a1.wav", "down/a2.wav", "down/a3.wav"] },
  "up": { "space": ["up/space.wav"] }
}
JSON

cat > "$packs/no-up/pack.json" <<'JSON'
{
  "name": "No up section",
  "license": "CC0-1.0",
  "keys": { "default": ["down/a1.wav", "down/a2.wav", "down/a3.wav"] }
}
JSON

# The same file (down/a1.wav) is in default and in space.
cat > "$packs/shared/pack.json" <<'JSON'
{
  "name": "Shared file",
  "license": "CC0-1.0",
  "keys": {
    "default": ["down/a1.wav", "down/a2.wav", "down/a3.wav"],
    "space": ["down/a1.wav"]
  }
}
JSON

cat > "$packs/extra-field/pack.json" <<'JSON'
{
  "name": "Extra field",
  "license": "CC0-1.0",
  "version": 7,
  "homepage": "https://example.invalid",
  "keys": { "default": ["down/a1.wav", "down/a2.wav", "down/a3.wav"] }
}
JSON

# The formats pack: every audio fixture format, plus the WAV named .ogg.
rmdir "$packs/formats/up" 2>/dev/null || true
mkdir -p "$packs/formats/audio"
cp "$audio"/tone-*.* "$audio/wav-named-ogg.ogg" "$packs/formats/audio/"
rmdir "$packs/formats/down" 2>/dev/null || true
files=$(cd "$packs/formats" && ls audio | sed 's|.*|"audio/&"|' | paste -sd, -)
cat > "$packs/formats/pack.json" <<JSON
{
  "name": "Formats",
  "license": "CC0-1.0",
  "keys": { "default": [$files] }
}
JSON

echo "fixtures written to $audio and $packs"
