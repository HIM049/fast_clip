# Reuse vcpkg's maintained FFmpeg build logic while narrowing this overlay
# port's default feature set in vcpkg.json.
include("${VCPKG_ROOT_DIR}/ports/ffmpeg/portfile.cmake")
