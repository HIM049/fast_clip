# 1. 创建临时 iconset 目录
mkdir -p assets/AppIcon.iconset

# 2. 生成各尺寸图标
sips -z 16 16     assets/app_icon_apple.png --out assets/AppIcon.iconset/icon_16x16.png
sips -z 32 32     assets/app_icon_apple.png --out assets/AppIcon.iconset/icon_16x16@2x.png
sips -z 32 32     assets/app_icon_apple.png --out assets/AppIcon.iconset/icon_32x32.png
sips -z 64 64     assets/app_icon_apple.png --out assets/AppIcon.iconset/icon_32x32@2x.png
sips -z 128 128   assets/app_icon_apple.png --out assets/AppIcon.iconset/icon_128x128.png
sips -z 256 256   assets/app_icon_apple.png --out assets/AppIcon.iconset/icon_128x128@2x.png
sips -z 256 256   assets/app_icon_apple.png --out assets/AppIcon.iconset/icon_256x256.png
sips -z 512 512   assets/app_icon_apple.png --out assets/AppIcon.iconset/icon_256x256@2x.png
sips -z 512 512   assets/app_icon_apple.png --out assets/AppIcon.iconset/icon_512x512.png
sips -z 1024 1024 assets/app_icon_apple.png --out assets/AppIcon.iconset/icon_512x512@2x.png

# 3. 打包生成 AppIcon.icns 并清理临时文件夹
iconutil -c icns assets/AppIcon.iconset -o assets/AppIcon.icns
rm -rf assets/AppIcon.iconset
