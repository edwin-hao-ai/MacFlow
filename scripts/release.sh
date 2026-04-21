#!/usr/bin/env bash
# MacFlow release build + notarization 脚本
# 用法: ./scripts/release.sh [arm|intel|universal]

set -euo pipefail

TARGET="${1:-arm}"
TEAM_ID="5XNDF727Y6"
SIGNING_ID="Developer ID Application: Beijing VGO Co;Ltd (${TEAM_ID})"
PROFILE_NAME="macflow-notary"

case "$TARGET" in
  arm)
    RUST_TARGET="aarch64-apple-darwin"
    ;;
  intel)
    RUST_TARGET="x86_64-apple-darwin"
    ;;
  universal)
    RUST_TARGET="universal-apple-darwin"
    ;;
  *)
    echo "用法: $0 [arm|intel|universal]" >&2
    exit 1
    ;;
esac

echo "==> 目标架构: $RUST_TARGET"
echo "==> 签名身份: $SIGNING_ID"

# 1. Tauri release build (自动用 tauri.conf.json 里配置的签名身份)
bun run tauri build --target "$RUST_TARGET"

APP_PATH="src-tauri/target/${RUST_TARGET}/release/bundle/macos/MacFlow.app"
DMG_PATH=$(ls src-tauri/target/${RUST_TARGET}/release/bundle/dmg/MacFlow_*.dmg | head -1)

echo "==> 生成的 .app: $APP_PATH"
echo "==> 生成的 DMG: $DMG_PATH"

# 2. 验证签名
echo "==> 验证签名..."
codesign --verify --deep --strict --verbose=2 "$APP_PATH"

# 3. 检查 notarytool 凭证是否已存
if ! xcrun notarytool history --keychain-profile "$PROFILE_NAME" >/dev/null 2>&1; then
  echo "==> notarytool 凭证未配置"
  echo ""
  echo "请先执行下面的命令配置凭证（只需一次，存进 Keychain）:"
  echo ""
  echo "  xcrun notarytool store-credentials $PROFILE_NAME \\"
  echo "    --apple-id YOUR_APPLE_ID@example.com \\"
  echo "    --team-id $TEAM_ID \\"
  echo "    --password YOUR_APP_SPECIFIC_PASSWORD"
  echo ""
  echo "App-Specific Password 从 https://appleid.apple.com → 登录和安全 → App 专用密码 生成"
  echo ""
  echo "配置完后重跑此脚本即可自动 notarize。"
  exit 0
fi

# 4. 提交到 Apple 公证
echo "==> 提交 DMG 到 Apple 公证..."
xcrun notarytool submit "$DMG_PATH" \
  --keychain-profile "$PROFILE_NAME" \
  --wait

# 5. Staple ticket 到 DMG 和 .app
echo "==> 装订公证票据..."
xcrun stapler staple "$DMG_PATH"
xcrun stapler staple "$APP_PATH"

# 6. 最终验证
echo "==> Gatekeeper 验证..."
spctl -a -vv -t install "$DMG_PATH" || true
spctl -a -vv "$APP_PATH" || true

echo ""
echo "✅ Release 完成"
echo "   DMG: $DMG_PATH"
echo "   .app: $APP_PATH"
ls -lh "$DMG_PATH" | awk '{print "   大小: " $5}'
