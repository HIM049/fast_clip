#!/usr/bin/env bash
set -euo pipefail

app_path="${1:?Usage: package-macos.sh <FastClip.app> <archive.zip>}"
archive_path="${2:?Usage: package-macos.sh <FastClip.app> <archive.zip>}"
ffmpeg_prefix="${FFMPEG_PREFIX:?FFMPEG_PREFIX must point to the Homebrew ffmpeg prefix}"
homebrew_prefix="$(brew --prefix)"
frameworks_path="$app_path/Contents/Frameworks"
executable_path="$app_path/Contents/MacOS/fast_clip"

test -d "$app_path"
test -f "$executable_path"

mkdir -p "$frameworks_path"
cp LICENSE "$app_path/Contents/Resources/LICENSE-fast_clip.txt"

declare -a queue=("$executable_path")
declare -A processed=()

is_homebrew_dependency() {
    [[ "$1" == "$homebrew_prefix/"* || "$1" == "$ffmpeg_prefix/"* ]]
}

while ((${#queue[@]})); do
    binary="${queue[0]}"
    queue=("${queue[@]:1}")

    [[ -n "${processed[$binary]:-}" ]] && continue
    processed[$binary]=1

    while IFS= read -r dependency; do
        is_homebrew_dependency "$dependency" || continue

        framework_name="$(basename "$dependency")"
        framework_path="$frameworks_path/$framework_name"
        if [[ ! -f "$framework_path" ]]; then
            cp -L "$dependency" "$framework_path"
            chmod u+w "$framework_path"
            queue+=("$framework_path")
        fi
        install_name_tool -change "$dependency" "@rpath/$framework_name" "$binary"
    done < <(otool -L "$binary" | tail -n +2 | awk '{print $1}')
done

install_name_tool -add_rpath "@executable_path/../Frameworks" "$executable_path" 2>/dev/null || true

for framework in "$frameworks_path"/*.dylib; do
    [[ -e "$framework" ]] || continue
    install_name_tool -id "@rpath/$(basename "$framework")" "$framework"
    install_name_tool -add_rpath "@loader_path" "$framework" 2>/dev/null || true
done

if otool -L "$executable_path" "$frameworks_path"/*.dylib | grep -E "($homebrew_prefix|/usr/local/(Cellar|opt))"; then
    echo "Homebrew dylib reference remains in the app bundle" >&2
    exit 1
fi

codesign --force --deep --sign - "$app_path"
codesign --verify --deep --strict --verbose=2 "$app_path"

rm -f "$archive_path"
mkdir -p "$(dirname "$archive_path")"
ditto -c -k --sequesterRsrc --keepParent "$app_path" "$archive_path"
