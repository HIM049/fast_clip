#!/usr/bin/env bash
set -euo pipefail

app_path="${1:?Usage: package-macos.sh <FastClip.app> <archive.zip>}"
archive_path="${2:?Usage: package-macos.sh <FastClip.app> <archive.zip>}"
ffmpeg_prefix="${FFMPEG_PREFIX:?FFMPEG_PREFIX must point to the Homebrew ffmpeg prefix}"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd "$script_dir/.." && pwd)"
frameworks_path="$app_path/Contents/Frameworks"
executable_path="$app_path/Contents/MacOS/fast_clip"

test -d "$app_path"
test -f "$executable_path"

for command in brew codesign ditto install_name_tool otool; do
    command -v "$command" >/dev/null 2>&1 || {
        echo "Required packaging command not found: $command" >&2
        exit 1
    }
done

homebrew_prefix="$(brew --prefix)"
test -d "$ffmpeg_prefix"

mkdir -p "$frameworks_path"
mkdir -p "$app_path/Contents/Resources"
cp "$project_root/LICENSE" "$app_path/Contents/Resources/LICENSE-fast_clip.txt"

declare -a queue=("$executable_path")
declare -a processed=()

is_homebrew_dependency() {
    [[ "$1" == "$homebrew_prefix/"* || "$1" == "$ffmpeg_prefix/"* ]]
}

was_processed() {
    local candidate="$1"
    local entry

    # macOS ships Bash 3.2, where an empty array is unset under `set -u`.
    for entry in "${processed[@]+"${processed[@]}"}"; do
        [[ "$entry" == "$candidate" ]] && return 0
    done
    return 1
}

queue_index=0
while ((queue_index < ${#queue[@]})); do
    binary="${queue[$queue_index]}"
    ((queue_index += 1))

    was_processed "$binary" && continue
    processed+=("$binary")

    dependency_output="$(otool -L "$binary")"
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
    done < <(printf '%s\n' "$dependency_output" | awk 'NR > 1 {print $1}')
done

install_name_tool -add_rpath "@executable_path/../Frameworks" "$executable_path" 2>/dev/null || true

declare -a bundle_binaries=("$executable_path")
for framework in "$frameworks_path"/*.dylib; do
    [[ -e "$framework" ]] || continue
    install_name_tool -id "@rpath/$(basename "$framework")" "$framework"
    install_name_tool -add_rpath "@loader_path" "$framework" 2>/dev/null || true
    bundle_binaries+=("$framework")
done

for binary in "${bundle_binaries[@]}"; do
    dependency_output="$(otool -L "$binary")"
    while IFS= read -r dependency; do
        if is_homebrew_dependency "$dependency" || [[ "$dependency" == /usr/local/Cellar/* || "$dependency" == /usr/local/opt/* ]]; then
            echo "Homebrew dylib reference remains in $binary: $dependency" >&2
            exit 1
        fi
    done < <(printf '%s\n' "$dependency_output" | awk 'NR > 1 {print $1}')
done

codesign --force --deep --sign - "$app_path"
codesign --verify --deep --strict --verbose=2 "$app_path"

rm -f "$archive_path"
mkdir -p "$(dirname "$archive_path")"
ditto -c -k --sequesterRsrc --keepParent "$app_path" "$archive_path"
test -s "$archive_path"
