#!/usr/bin/env bash
set -euo pipefail

binary_argument="${1:?Usage: package-linux.sh <release-binary>}"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd "$script_dir/.." && pwd)"
bundle_dir="$project_root/target/release/bundle/appimage"
app_dir="$bundle_dir/FastClip.AppDir"
output_dir="$bundle_dir/output"
verify_dir="$bundle_dir/verify"
tools_dir="$bundle_dir/tools"
final_image="$bundle_dir/FastClip-linux-x86_64.AppImage"
desktop_file="$project_root/packaging/linux/com.him049.fastclip.desktop"
source_icon="$project_root/assets/app_icon.png"
packaging_icon="$bundle_dir/com.him049.fastclip.png"
license_file="$project_root/LICENSE"
linuxdeploy="$tools_dir/linuxdeploy-x86_64.AppImage"
linuxdeploy_url="https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage"

case "$binary_argument" in
    /*) binary_path="$binary_argument" ;;
    *) binary_path="$project_root/$binary_argument" ;;
esac

for command in chmod convert curl dpkg-query file find grep head identify install ldd lddtree mkdir mv readlink rm sed sort uname; do
    command -v "$command" >/dev/null 2>&1 || {
        echo "Required packaging command not found: $command" >&2
        exit 1
    }
done

for required_file in "$binary_path" "$desktop_file" "$source_icon" "$license_file"; do
    if [[ ! -f "$required_file" ]]; then
        echo "Required packaging input not found: $required_file" >&2
        exit 1
    fi
done

if [[ "$(uname -m)" != "x86_64" ]]; then
    echo "The Linux AppImage package currently supports x86_64 only" >&2
    exit 1
fi

rm -rf "$bundle_dir"
mkdir -p \
    "$app_dir/usr/share/licenses/fast_clip/third-party" \
    "$output_dir" \
    "$tools_dir" \
    "$verify_dir"

install -m 644 "$license_file" "$app_dir/usr/share/licenses/fast_clip/LICENSE"
convert "$source_icon" -filter Lanczos -resize 512x512 "$packaging_icon"
if [[ "$(identify -format '%wx%h' "$packaging_icon")" != "512x512" ]]; then
    echo "Failed to create a 512x512 AppImage icon" >&2
    exit 1
fi

# Preserve Debian copyright notices for every package in the recursive ELF tree.
while IFS= read -r dependency; do
    [[ -e "$dependency" ]] || continue
    resolved_dependency="$(readlink -f "$dependency")"
    owners="$(
        {
            dpkg-query -S "$dependency" 2>/dev/null || true
            if [[ "$resolved_dependency" != "$dependency" ]]; then
                dpkg-query -S "$resolved_dependency" 2>/dev/null || true
            fi
        } | sed -n 's/: .*//p' | sort -u
    )"

    while IFS= read -r owner; do
        [[ -n "$owner" ]] || continue
        package_name="${owner%%:*}"
        copyright_file="/usr/share/doc/$package_name/copyright"
        if [[ -f "$copyright_file" ]]; then
            install -m 644 \
                "$copyright_file" \
                "$app_dir/usr/share/licenses/fast_clip/third-party/$package_name.copyright"
        fi
    done <<< "$owners"
done < <(lddtree -l "$binary_path")

if ! find "$app_dir/usr/share/licenses/fast_clip/third-party" -type f -print -quit | grep -q .; then
    echo "No third-party dependency licenses were collected" >&2
    exit 1
fi

curl --fail --location --retry 3 --retry-all-errors \
    --output "$linuxdeploy" \
    "$linuxdeploy_url"
chmod +x "$linuxdeploy"

(
    cd "$output_dir"
    ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$linuxdeploy" \
        --appdir "$app_dir" \
        --executable "$binary_path" \
        --desktop-file "$desktop_file" \
        --icon-file "$packaging_icon" \
        --output appimage
)

declare -a generated_images=()
while IFS= read -r image; do
    generated_images+=("$image")
done < <(find "$output_dir" -maxdepth 1 -type f -name '*.AppImage' -print)

if ((${#generated_images[@]} != 1)); then
    echo "Expected exactly one generated AppImage, found ${#generated_images[@]}" >&2
    exit 1
fi

mv "${generated_images[0]}" "$final_image"
chmod +x "$final_image"

test -s "$final_image"
test -x "$final_image"
file "$final_image" | grep -Eq 'ELF 64-bit LSB.*x86-64'

(
    cd "$verify_dir"
    "$final_image" --appimage-extract >/dev/null
)

extracted_root="$verify_dir/squashfs-root"
extracted_binary="$extracted_root/usr/bin/fast_clip"
test -x "$extracted_binary"
test -f "$extracted_root/com.him049.fastclip.desktop"
test -f "$extracted_root/usr/share/licenses/fast_clip/LICENSE"
find "$extracted_root/usr/share/icons" -type f -name 'com.him049.fastclip.png' -print -quit | grep -q .
find "$extracted_root/usr/share/licenses/fast_clip/third-party" -type f -print -quit | grep -q .

ldd_output="$(ldd "$extracted_binary")"
if grep -Fq 'not found' <<< "$ldd_output"; then
    printf '%s\n' "$ldd_output" >&2
    echo "The extracted AppImage contains unresolved dynamic dependencies" >&2
    exit 1
fi

for library_name in libavcodec libavformat libavutil libswresample libswscale; do
    library_line="$(grep -E "${library_name}\\.so" <<< "$ldd_output" | head -n 1 || true)"
    if [[ -z "$library_line" || "$library_line" != *"$extracted_root"* ]]; then
        printf '%s\n' "$ldd_output" >&2
        echo "$library_name is not resolved from inside the AppImage" >&2
        exit 1
    fi
done

declare -a forbidden_patterns=(
    'ld-linux*.so*'
    'libc.so*'
    'libdl.so*'
    'libpthread.so*'
    'libvulkan_intel.so*'
    'libvulkan_radeon.so*'
    'libGLX_nvidia.so*'
    '*_dri.so*'
)

for pattern in "${forbidden_patterns[@]}"; do
    forbidden_file="$(find "$extracted_root" -type f -name "$pattern" -print -quit)"
    if [[ -n "$forbidden_file" ]]; then
        echo "The AppImage contains a forbidden host library: $forbidden_file" >&2
        exit 1
    fi
done

printf 'Created %s\n' "$final_image"
