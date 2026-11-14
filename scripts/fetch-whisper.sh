#!/usr/bin/env bash
# Download Whisper weights in the Candle layout expected by karaoke-cli.
#
#   ./scripts/fetch-whisper.sh [model] [destination]
#   FORCE=1 ./scripts/fetch-whisper.sh small /models/whisper-small
#
# The CLI itself never downloads anything; this is an explicit one-off helper.
set -euo pipefail

model="${1:-base}"
dest="${2:-$PWD/whisper-$model}"
repo="openai/whisper-$model"

case "$model" in
  tiny|tiny.en|base|base.en|small|small.en|medium|medium.en|large-v1|large-v2|large-v3) ;;
  *)
    echo "unknown whisper model '$model'" >&2
    echo "choose one of: tiny tiny.en base base.en small small.en medium medium.en large-v1 large-v2 large-v3" >&2
    exit 2
    ;;
esac

mkdir -p "$dest"
echo "repo:        $repo"
echo "destination: $dest"

for file in config.json tokenizer.json model.safetensors; do
  if [ -s "$dest/$file" ] && [ "${FORCE:-0}" != 1 ]; then
    echo "skip  $file (already present; FORCE=1 to replace)"
    continue
  fi
  echo "get   $file"
  curl -fL --retry 3 --retry-delay 5 -o "$dest/$file.part" \
    "https://huggingface.co/$repo/resolve/main/$file"
  mv -f "$dest/$file.part" "$dest/$file"
done

# An error page from the CDN would still be a "successful" download, so check
# that the JSON parses (when jq is available) and the weights are a plausible
# checkpoint size.
if command -v jq >/dev/null 2>&1; then
  for file in config.json tokenizer.json; do
    jq -e . "$dest/$file" >/dev/null
  done
fi

size=$(wc -c < "$dest/model.safetensors")
if [ "$size" -lt 1048576 ]; then
  echo "model.safetensors is only $size bytes - not a checkpoint" >&2
  exit 1
fi

ls -l "$dest"
echo
echo "Done. Use it with:"
echo "  karaoke-cli song.mp3 --model_filename <model>.safetensors --config <profile>.yaml \\"
echo "    --output_dir out --transcribe --whisper_model_dir \"$dest\" --language ru"
