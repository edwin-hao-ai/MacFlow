#!/usr/bin/env bash
# 手动对已打包好的 .app / DMG 签名 + notarize
# 用法：./scripts/sign.sh [arm|intel|universal]
#
# 背景：Apple 的 timestamp.apple.com 服务会间歇性不可用，导致 Tauri 一体化
# 构建中断。这个脚本把「编译」和「签名+公证」拆开，失败可单独重跑签名。

set -euo pipefail

TARGET="${1:-arm}"
TEAM_ID="5XNDF727Y6"
SIGNING_ID="Developer ID Application: Beijing VGO Co;Ltd (${TEAM_ID})"
PROFILE_NAME="macflow-notary"
ENTITLEMENTS="src-tauri/entitlements.plist"

case "$TARGET" in
  arm)       RUST_TARGET="aarch64-apple-darwin" ;;
  intel)     RUST_TARGET="x86_64-apple-darwin" ;;
  universal) RUST_TARGET="universal-apple-darwin" ;;
  *) echo "用法: $0 [arm|intel|universal]"; exit 1 ;;
esac

APP_PATH="src-tauri/target/${RUST_TARGET}/release/bundle/macos/MacFlow.app"
DMG_PATH=$(ls src-tauri/target/${RUST_TARGET}/release/bundle/dmg/MacFlow_*.dmg 2>/dev/null | head -1 || true)

if [ ! -d "$APP_PATH" ]; then
  echo "错误: 找不到 $APP_PATH，请先跑 bun run tauri build --target $RUST_TARGET"
  exit 1
fi

# 探测 Apple timestamp 服务
TIMESTAMP_FLAG="--timestamp"
if ! codesign --force --options runtime --timestamp \
     --sign "$SIGNING_ID" \
     "$APP_PATH/Contents/MacOS/macflow" >/dev/null 2>&1; then
  echo "⚠️  timestamp.apple.com 不可用，使用无时间戳签名（无法通过公证，但本地可用）"
  TIMESTAMP_FLAG="--timestamp=none"
fi

echo "==> 注入 Info.plist 自定义键（必须在签名之前，否则签名失效）..."
INFO_PLIST="$APP_PATH/Contents/Info.plist"
# NSAppleEventsUsageDescription：osascript 'tell application X to quit' 必需
# Hardened Runtime 下没有这个 key，AppleEvents 调用会被静默拒绝（macOS 14+）
if ! /usr/libexec/PlistBuddy -c "Print :NSAppleEventsUsageDescription" "$INFO_PLIST" >/dev/null 2>&1; then
  /usr/libexec/PlistBuddy -c "Add :NSAppleEventsUsageDescription string 'MacFlow 需要发送 AppleEvent 来优雅退出其他应用程序。'" "$INFO_PLIST"
  echo "    ✓ 已注入 NSAppleEventsUsageDescription"
else
  echo "    ✓ NSAppleEventsUsageDescription 已存在，跳过"
fi

echo "==> 签名 .app 内所有二进制..."
for f in "$APP_PATH/Contents/MacOS"/*; do
  if [ -f "$f" ] && [ -x "$f" ]; then
    codesign --force --options runtime $TIMESTAMP_FLAG \
      --entitlements "$ENTITLEMENTS" \
      --sign "$SIGNING_ID" "$f" 2>&1 | grep -v "^$" || true
  fi
done

echo "==> 签名整个 .app..."
codesign --force --options runtime $TIMESTAMP_FLAG \
  --entitlements "$ENTITLEMENTS" \
  --sign "$SIGNING_ID" "$APP_PATH"

echo "==> 验证签名..."
codesign --verify --deep --strict --verbose=2 "$APP_PATH"

if [ -n "$DMG_PATH" ]; then
  echo "==> 签名 DMG..."
  codesign --force $TIMESTAMP_FLAG --sign "$SIGNING_ID" "$DMG_PATH"
  codesign --verify --verbose "$DMG_PATH"
fi

if [ "$TIMESTAMP_FLAG" = "--timestamp=none" ]; then
  echo ""
  echo "⚠️  本次签名无时间戳 —— 无法通过 Apple 公证。"
  echo "   Apple timestamp 服务恢复后，重跑 ./scripts/sign.sh $TARGET 即可自动公证。"
  exit 0
fi

# notarize
if xcrun notarytool history --keychain-profile "$PROFILE_NAME" >/dev/null 2>&1; then
  if [ -n "$DMG_PATH" ]; then
    echo "==> 提交 DMG 到 Apple 公证..."
    xcrun notarytool submit "$DMG_PATH" \
      --keychain-profile "$PROFILE_NAME" \
      --wait

    echo "==> 装订公证票据..."
    xcrun stapler staple "$DMG_PATH"
    xcrun stapler staple "$APP_PATH"

    echo "==> Gatekeeper 最终验证..."
    spctl -a -vv -t install "$DMG_PATH" || true
    spctl -a -vv "$APP_PATH" || true
  fi
else
  echo ""
  echo "==> notarytool 凭证未配置。首次执行一次下面的命令："
  echo ""
  echo "  xcrun notarytool store-credentials $PROFILE_NAME \\"
  echo "    --apple-id YOUR_APPLE_ID@example.com \\"
  echo "    --team-id $TEAM_ID \\"
  echo "    --password YOUR_APP_SPECIFIC_PASSWORD"
  echo ""
  echo "App-Specific Password 到 https://appleid.apple.com 登录后生成。"
fi

echo ""
echo "✅ 签名流程结束"
ls -lh "$APP_PATH" "$DMG_PATH" 2>/dev/null | awk '{print "   " $5 "  " $9}'
