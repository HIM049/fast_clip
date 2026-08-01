#!/usr/bin/env bash
set -euo pipefail

binary_argument="${1:?Usage: package-macos.sh <release-binary>}"
ffmpeg_prefix="${FFMPEG_PREFIX:?FFMPEG_PREFIX must point to the Homebrew ffmpeg prefix}"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd "$script_dir/.." && pwd)"
libraries_path="$project_root/libraries"
dmg_path="$project_root/target/release/bundle/dmg/FastClip.dmg"
mount_point=""
binary_backup=""

case "$binary_argument" in
    /*) binary_path="$binary_argument" ;;
    *) binary_path="$project_root/$binary_argument" ;;
esac

if [[ "$project_root" == "/" || "$libraries_path" != "$project_root/libraries" ]]; then
    echo "Refusing to use an unsafe temporary libraries path: $libraries_path" >&2
    exit 1
fi

cleanup() {
    if [[ -n "$mount_point" ]]; then
        hdiutil detach "$mount_point" >/dev/null 2>&1 || true
    fi
    if [[ -n "$binary_backup" && -f "$binary_backup" ]]; then
        cp -p "$binary_backup" "$binary_path"
        rm -f "$binary_backup"
    fi
    rm -rf "$libraries_path"
}
trap cleanup EXIT

for command in awk basename brew cargo chmod codesign cp grep hdiutil install_name_tool mkdir mktemp otool rm tail; do
    command -v "$command" >/dev/null 2>&1 || {
        echo "Required packaging command not found: $command" >&2
        exit 1
    }
done

test -f "$binary_path"
test -d "$ffmpeg_prefix"

homebrew_prefix="$(brew --prefix)"
backup_candidate="$(mktemp "${TMPDIR:-/tmp}/fastclip-binary.XXXXXX")"
if ! cp -p "$binary_path" "$backup_candidate"; then
    rm -f "$backup_candidate"
    exit 1
fi
binary_backup="$backup_candidate"
rm -rf "$libraries_path"
mkdir -p "$libraries_path"

declare -a queue=("$binary_path")
declare -a processed=()

is_homebrew_dependency() {
    [[ "$1" == "$homebrew_prefix/"* || "$1" == "$ffmpeg_prefix/"* ]]
}

is_forbidden_dependency() {
    is_homebrew_dependency "$1" || [[ "$1" == /usr/local/Cellar/* || "$1" == /usr/local/opt/* ]]
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

declare -a bundled_binaries=("$binary_path")
for library in "$libraries_path"/*; do
    [[ -f "$library" ]] || continue
    library_name="$(basename "$library")"
    install_name_tool -id "@rpath/libraries/$library_name" "$library"
    has_rpath "$library" "@loader_path/.." || install_name_tool -add_rpath "@loader_path/.." "$library"
    bundled_binaries+=("$library")
done

if ((${#bundled_binaries[@]} == 1)); then
    echo "No Homebrew dynamic libraries were collected from $binary_path" >&2
    exit 1
fi

has_rpath "$binary_path" "@executable_path/../Resources" \
    || install_name_tool -add_rpath "@executable_path/../Resources" "$binary_path"

for binary in "${bundled_binaries[@]}"; do
    dependency_output="$(dependencies_of "$binary")"
    while IFS= read -r dependency; do
        if is_forbidden_dependency "$dependency"; then
            echo "Homebrew dylib reference remains in $binary: $dependency" >&2
            exit 1
        fi
    done <<< "$dependency_output"

    codesign --force --sign - "$binary"
    codesign --verify --strict --verbose=2 "$binary"
done

dependencies_of "$binary_path" | grep -Fq '@rpath/libraries/' || {
    echo "The release binary does not reference the bundled libraries path" >&2
    exit 1
}

cd "$project_root"
CARGO_BUNDLE_SKIP_BUILD=1 cargo bundle --release --format dmg

test -s "$dmg_path"
hdiutil verify "$dmg_path"

attach_output="$(hdiutil attach "$dmg_path" -readonly -nobrowse -noverify -noautoopen)"
mount_point="$(printf '%s\n' "$attach_output" | awk -F '\t' '$NF ~ /^\/Volumes\// { print $NF }' | tail -n 1)"
if [[ -z "$mount_point" ]]; then
    echo "Could not determine the mounted DMG path" >&2
    exit 1
fi

mounted_app="$mount_point/FastClip.app"
mounted_binary="$mounted_app/Contents/MacOS/fast_clip"
mounted_libraries="$mounted_app/Contents/Resources/libraries"
test -f "$mounted_binary"
test -d "$mounted_libraries"
test -f "$mounted_app/Contents/Resources/LICENSE"

declare -a mounted_binaries=("$mounted_binary")
for library in "$mounted_libraries"/*; do
    [[ -f "$library" ]] || continue
    mounted_binaries+=("$library")
done

if ((${#mounted_binaries[@]} == 1)); then
    echo "The DMG does not contain any bundled dynamic libraries" >&2
    exit 1
fi

if ((${#mounted_binaries[@]} != ${#bundled_binaries[@]})); then
    echo "The DMG library count does not match the staged library count" >&2
    exit 1
fi

for binary in "${mounted_binaries[@]}"; do
    dependency_output="$(dependencies_of "$binary")"
    while IFS= read -r dependency; do
        if is_forbidden_dependency "$dependency"; then
            echo "Homebrew dylib reference remains in the DMG: $binary: $dependency" >&2
            exit 1
        fi
    done <<< "$dependency_output"
    # The app is not bundle-signed, so verifying a Mach-O inside it makes
    # codesign require a missing Contents/_CodeSignature/CodeResources seal.
    # codesign --verify --strict --verbose=2 "$binary"
done

dependencies_of "$mounted_binary" | grep -Fq '@rpath/libraries/' || {
    echo "The DMG executable does not reference the bundled libraries path" >&2
    exit 1
}

hdiutil detach "$mount_point"
mount_point=""
