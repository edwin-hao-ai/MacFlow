#!/usr/bin/env bash
# MacSlim 更新发布脚本
#
# 用法:
#   ./scripts/publish-update.sh <version> "<release notes>"
#
# 例如:
#   ./scripts/publish-update.sh 0.2.0 "修复若干安全问题，新增 Rust 缓存扫描"
#
# 前置:
#   1. 已经跑过一次 release 打包 (scripts/release.sh arm)
#   2. 环境变量里有 TAURI_SIGNING_PRIVATE_KEY 和 TAURI_SIGNING_PRIVATE_KEY_PASSWORD
#      （或者 TAURI_SIGNING_PRIVATE_KEY_PATH 指向密钥文件）
#
# 输出:
#   - landing/updates/darwin-aarch64/<version>.json  更新清单（服务端用）
#   - landing/downloads/MacSlim_<version>_aarch64.dmg  最新 DMG
#
# 部署:
#   把 landing/ 整目录推到 https://edwin-hao-ai.github.io/MacSlim 即可，用户客户端自动检测新版本。

set -euo pipefail

VERSION="${1:-}"
NOTES="${2:-}"

if [ -z "$VERSION" ]; then
  echo "用法: $0 <version> \"<release notes>\"" >&2
  exit 1
fi

if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ] && [ -z "${TAURI_SIGNING_PRIVATE_KEY_PATH:-}" ]; then
  # 默认从 ~/.tauri/macslim-updater.key 读
  if [ -f "$HOME/.tauri/macslim-updater.key" ]; then
    export TAURI_SIGNING_PRIVATE_KEY_PATH="$HOME/.tauri/macslim-updater.key"
    export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-macslim-dev-pw}"
    echo "==> 使用默认密钥 $TAURI_SIGNING_PRIVATE_KEY_PATH"
  else
    echo "错误: 未找到 updater 私钥" >&2
    echo "请先运行: bun tauri signer generate -w ~/.tauri/macslim-updater.key --force --ci --password 你的密码" >&2
    exit 1
  fi
fi

DMG_SRC="src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/MacSlim_${VERSION}_aarch64.dmg"
if [ ! -f "$DMG_SRC" ]; then
  echo "错误: 找不到 $DMG_SRC" >&2
  echo "请先执行: ./scripts/release.sh arm" >&2
  exit 1
fi

echo "==> 签名 DMG 用于 Tauri Updater..."
# tauri 的 signer sign 把 .dmg 签成带 .sig 的 minisign 签名
bun tauri signer sign "$DMG_SRC" 2>&1 | tail -5

SIG_FILE="${DMG_SRC}.sig"
if [ ! -f "$SIG_FILE" ]; then
  echo "错误: 签名文件 $SIG_FILE 未生成" >&2
  exit 1
fi

SIGNATURE=$(cat "$SIG_FILE")
PUB_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

# 目标 URL 在 tauri.conf.json 里配置为 https://edwin-hao-ai.github.io/MacSlim/updates/{{target}}-{{arch}}/{{current_version}}.json
# 但 Tauri 2 的 target 参数实际是 darwin-aarch64 / darwin-x86_64 / windows-x86_64 / linux-x86_64
# 我们把 manifest 发布到 darwin-aarch64/<prev-version>.json （服务端静态文件）
# 而 DMG 本身放到 downloads/
MANIFEST_DIR="landing/updates/darwin-aarch64"
mkdir -p "$MANIFEST_DIR"
mkdir -p landing/downloads

# 拷贝 DMG 到 landing
DMG_BASENAME=$(basename "$DMG_SRC")
cp "$DMG_SRC" "landing/downloads/${DMG_BASENAME}"

# 生成 manifest
# 注意：Tauri updater 检查时发送 {{current_version}} = 当前安装版本
# 我们的 endpoint 设计：https://.../<TARGET>-<ARCH>/<current_version>.json
# 所以对「老版本升级到新版本」，需要为每个老版本写一个 manifest
# 简化方案：写 latest.json，让用户 endpoint 指向 latest.json
# 但 tauri.conf.json 的 {{current_version}} 模板已定，我们生成多个 manifest：
# - latest.json （可手动访问）
# - <版本号>.json （给当前已部署的版本检查）

cat > "${MANIFEST_DIR}/latest.json" <<EOF
{
  "version": "${VERSION}",
  "notes": ${NOTES:-"\"\""},
  "pub_date": "${PUB_DATE}",
  "platforms": {
    "darwin-aarch64": {
      "signature": "${SIGNATURE}",
      "url": "https://github.com/edwin-hao-ai/MacSlim/releases/latest/download/${DMG_BASENAME}"
    }
  }
}
EOF

# notes 需要 JSON 转义，用 python 规范化
python3 - <<PY
import json, pathlib
manifest_path = pathlib.Path("${MANIFEST_DIR}/latest.json")
data = {
  "version": "${VERSION}",
  "notes": """${NOTES}""",
  "pub_date": "${PUB_DATE}",
  "platforms": {
    "darwin-aarch64": {
      "signature": """${SIGNATURE}""",
      "url": "https://github.com/edwin-hao-ai/MacSlim/releases/latest/download/${DMG_BASENAME}"
    }
  }
}
manifest_path.write_text(json.dumps(data, indent=2, ensure_ascii=False))
PY

# 也为每个已知老版本生成相同内容的 manifest（<oldversion>.json 指向 latest）
# 这里手工列出主要老版本；实际 CI 可以枚举
for old_ver in 0.1.0; do
  cp "${MANIFEST_DIR}/latest.json" "${MANIFEST_DIR}/${old_ver}.json"
done

echo ""
echo "✅ 更新包已生成"
echo "   DMG:       landing/downloads/${DMG_BASENAME}"
echo "   Manifest:  ${MANIFEST_DIR}/latest.json"
echo "   Signature: $(wc -c < "$SIG_FILE") bytes"
echo ""
echo "下一步：把 landing/ 整个目录部署到 https://edwin-hao-ai.github.io/MacSlim"
echo "已安装 MacSlim 的用户在「设置」→「检查更新」会看到新版本"
