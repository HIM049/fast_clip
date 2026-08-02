#!/usr/bin/env bash
set -euo pipefail

binary_argument="${1:?Usage: package-macos.sh <release-binary>}"
ffmpeg_prefix="${FFMPEG_PREFIX:?FFMPEG_PREFIX must point to the Homebrew ffmpeg prefix}"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd "$script_dir/.." && pwd)"
app_path="$project_root/target/release/bundle/osx/FastClip.app"
dmg_path="$project_root/target/release/bundle/dmg/FastClip.dmg"
libraries_path="$app_path/Contents/Frameworks/libraries"
mount_point=""
dmg_workspace=""

case "$binary_argument" in
    /*) binary_path="$binary_argument" ;;
    *) binary_path="$project_root/$binary_argument" ;;
esac
binary_name="$(basename "$binary_path")"
app_binary="$app_path/Contents/MacOS/$binary_name"

cleanup() {
    if [[ -n "$mount_point" ]]; then
        hdiutil detach "$mount_point" >/dev/null 2>&1 || true
    fi
    if [[ -n "$dmg_workspace" && -d "$dmg_workspace" ]]; then
        rm -rf "$dmg_workspace"
    fi
}
trap cleanup EXIT

for command in awk basename brew cargo chmod codesign cp ditto du grep hdiutil install_name_tool ln mkdir mktemp otool rm sync tail; do
    command -v "$command" >/dev/null 2>&1 || {
        echo "Required packaging command not found: $command" >&2
        exit 1
    }
done

test -f "$binary_path"
test -d "$ffmpeg_prefix"

homebrew_prefix="$(brew --prefix)"

is_homebrew_dependency() {
    [[ "$1" == "$homebrew_prefix/"* || "$1" == "$ffmpeg_prefix/"* ]]
}

is_forbidden_dependency() {
    is_homebrew_dependency "$1" \
        || [[ "$1" == /opt/homebrew/Cellar/* ]] \
        || [[ "$1" == /opt/homebrew/opt/* ]] \
        || [[ "$1" == /usr/local/Cellar/* ]] \
        || [[ "$1" == /usr/local/opt/* ]]
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

has_rpath() {
    local binary="$1"
    local expected="$2"

    otool -l "$binary" \
        | awk '$1 == "cmd" && $2 == "LC_RPATH" { getline; getline; if ($1 == "path") print $2 }' \
        | grep -Fqx "$expected"
}

dependencies_of() {
    local binary="$1"
    local output

    output="$(otool -L "$binary")"
    printf '%s\n' "$output" | awk 'NR > 1 {print $1}'
}

verify_dependencies() {
    local binary="$1"
    local dependency
    local dependency_output

    dependency_output="$(dependencies_of "$binary")"
    while IFS= read -r dependency; do
        if is_forbidden_dependency "$dependency"; then
            echo "Homebrew dylib reference remains in $binary: $dependency" >&2
            exit 1
        fi
    done <<< "$dependency_output"
}

cd "$project_root"
CARGO_BUNDLE_SKIP_BUILD=1 cargo bundle --release --format osx

test -f "$app_binary"
rm -rf "$libraries_path"
mkdir -p "$libraries_path"

declare -a queue=("$app_binary")
declare -a processed=()
queue_index=0

while ((queue_index < ${#queue[@]})); do
    binary="${queue[$queue_index]}"
    ((queue_index += 1))

    was_processed "$binary" && continue
    processed+=("$binary")

    dependency_output="$(dependencies_of "$binary")"
    while IFS= read -r dependency; do
        is_homebrew_dependency "$dependency" || continue

        library_name="$(basename "$dependency")"
        library_path="$libraries_path/$library_name"
        if [[ ! -f "$library_path" ]]; then
            cp -L "$dependency" "$library_path"
            chmod u+w "$library_path"
            queue+=("$library_path")
        fi
        install_name_tool -change "$dependency" "@rpath/libraries/$library_name" "$binary"
    done <<< "$dependency_output"
done

declare -a bundled_libraries=()
for library in "$libraries_path"/*; do
    [[ -f "$library" ]] || continue
    library_name="$(basename "$library")"
    install_name_tool -id "@rpath/libraries/$library_name" "$library"
    has_rpath "$library" "@loader_path/.." || install_name_tool -add_rpath "@loader_path/.." "$library"
    bundled_libraries+=("$library")
done

if [[ -z "${bundled_libraries[0]+set}" ]]; then
    echo "No Homebrew dynamic libraries were collected from $app_binary" >&2
    exit 1
fi

has_rpath "$app_binary" "@executable_path/../Frameworks" \
    || install_name_tool -add_rpath "@executable_path/../Frameworks" "$app_binary"

verify_dependencies "$app_binary"
for library in "${bundled_libraries[@]}"; do
    verify_dependencies "$library"
    codesign --force --sign - "$library"
    codesign --verify --strict --verbose=2 "$library"
done

dependencies_of "$app_binary" | grep -Fq '@rpath/libraries/' || {
    echo "The release binary does not reference the bundled libraries path" >&2
    exit 1
}

# Sign the complete bundle only after every executable and resource is final.
codesign --force --deep --sign - "$app_path"
codesign --verify --deep --strict --verbose=2 "$app_path"

dmg_workspace="$(mktemp -d "${TMPDIR:-/tmp}/fastclip-dmg.XXXXXX")"
working_dmg="$dmg_workspace/FastClip-rw.dmg"
working_mount="$dmg_workspace/mount"
app_size_kb="$(du -sk "$app_path" | awk '{print $1}')"
image_size_kb=$((app_size_kb + app_size_kb / 5 + 32768))

mkdir -p "$working_mount"
hdiutil create \
    -size "${image_size_kb}k" \
    -fs HFS+ \
    -volname FastClip \
    "$working_dmg"

mount_point="$working_mount"
hdiutil attach \
    "$working_dmg" \
    -mountpoint "$mount_point" \
    -nobrowse \
    -noverify \
    -noautoopen
ditto "$app_path" "$mount_point/FastClip.app"
ln -s /Applications "$mount_point/Applications"
sync
hdiutil detach "$mount_point"
mount_point=""

mkdir -p "$(dirname "$dmg_path")"
rm -f "$dmg_path"
hdiutil convert \
    "$working_dmg" \
    -format UDZO \
    -imagekey zlib-level=9 \
    -o "$dmg_path"

test -s "$dmg_path"
hdiutil verify "$dmg_path"

attach_output="$(hdiutil attach "$dmg_path" -readonly -nobrowse -noverify -noautoopen)"
mount_point="$(printf '%s\n' "$attach_output" | awk -F '\t' '$NF ~ /^\/Volumes\// { print $NF }' | tail -n 1)"
if [[ -z "$mount_point" ]]; then
    echo "Could not determine the mounted DMG path" >&2
    exit 1
fi

mounted_app="$mount_point/FastClip.app"
mounted_binary="$mounted_app/Contents/MacOS/$binary_name"
mounted_libraries="$mounted_app/Contents/Frameworks/libraries"
test -f "$mounted_binary"
test -d "$mounted_libraries"
test -f "$mounted_app/Contents/Resources/LICENSE"
test -L "$mount_point/Applications"

declare -a mounted_binaries=("$mounted_binary")
for library in "$mounted_libraries"/*; do
    [[ -f "$library" ]] || continue
    mounted_binaries+=("$library")
done

if ((${#mounted_binaries[@]} != ${#bundled_libraries[@]} + 1)); then
    echo "The DMG library count does not match the signed app" >&2
    exit 1
fi

for binary in "${mounted_binaries[@]}"; do
    verify_dependencies "$binary"
done

dependencies_of "$mounted_binary" | grep -Fq '@rpath/libraries/' || {
    echo "The DMG executable does not reference the bundled libraries path" >&2
    exit 1
}

codesign --verify --deep --strict --verbose=2 "$mounted_app"

hdiutil detach "$mount_point"
mount_point=""
