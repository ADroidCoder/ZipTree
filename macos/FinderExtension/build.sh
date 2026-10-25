#!/bin/bash
# 编译访达右键菜单扩展，输出到 target/finder/ZipTreeFinder.appex，打包时由 tauri.conf.json 放进 ZipTree.app/Contents/PlugIns。
# 用法：macos/FinderExtension/build.sh [目标架构，例如 aarch64-apple-darwin]，不传时编译当前机器的架构
set -euo pipefail
cd "$(dirname "$0")"
root=../..
out=$root/target/finder/ZipTreeFinder.appex
version=$(sed -n 's/^  "version": "\(.*\)",$/\1/p' $root/src-tauri/tauri.conf.json)

case "${1:-${TAURI_ENV_TARGET_TRIPLE:-}}" in
  x86_64-*) arch=x86_64 ;;
  aarch64-*) arch=arm64 ;;
  *) arch=$(uname -m) ;;
esac

rm -rf "$out"
mkdir -p "$out/Contents/MacOS"
sed "s/__VERSION__/$version/g" Info.plist > "$out/Contents/Info.plist"
# 扩展的入口是系统提供的 NSExtensionMain
xcrun swiftc FinderSync.swift \
  -module-name ZipTreeFinder \
  -target "$arch-apple-macos11.0" \
  -application-extension \
  -O \
  -framework Cocoa -framework FinderSync \
  -Xlinker -e -Xlinker _NSExtensionMain \
  -o "$out/Contents/MacOS/ZipTreeFinder"
# 没有开发者证书时用临时签名（“-”），本机可以用；正式发布需要用 Developer ID 签名
codesign --force --sign "${APPLE_SIGNING_IDENTITY:--}" --entitlements entitlements.plist --options runtime "$out"
echo "已生成 $out ($arch)"
