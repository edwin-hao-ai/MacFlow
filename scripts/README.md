# 发布脚本

| 脚本 | 做什么 | 什么时候用 |
| :--- | :--- | :--- |
| `release.sh` | 编译 + 代码签名（Apple Developer ID） | 每次要给用户下载新 DMG 时 |
| `sign.sh` | 单独跑代码签名（探活 timestamp 服务） | `release.sh` 的签名步骤失败后重试 |
| `publish-update.sh` | 给已打好的 DMG 签 Updater 签名 + 生成 manifest | 发布自动更新 |

## 发版工作流

```bash
# 1. 更新版本号
vim src-tauri/Cargo.toml src-tauri/tauri.conf.json package.json

# 2. 编译 + Apple 代码签名
./scripts/release.sh arm

# 3. Updater 签名 + 生成 manifest
./scripts/publish-update.sh 0.2.0 "本次更新：XXXX"

# 4. 部署 landing/ 到 GitHub Pages（docs/ 目录推到 main 分支即可）
rsync -avz landing/ <git push docs/ to GitHub Pages>
```

## 密钥管理

Updater 签名密钥：`~/.tauri/macslim-updater.key`（.pub 是公钥，可提交到源码；私钥**绝不提交**）。

公钥已经嵌入 `src-tauri/tauri.conf.json` 的 `plugins.updater.pubkey`。更换公钥需要重新发布所有平台的初始安装包，老用户才能验证新签名。

## Apple 签名

Developer ID Application: **Beijing VGO Co;Ltd (Team 5XNDF727Y6)**，已在 Keychain 里。
首次公证：

```bash
xcrun notarytool store-credentials macslim-notary \
  --apple-id YOUR_APPLE_ID@example.com \
  --team-id 5XNDF727Y6 \
  --password APP_SPECIFIC_PASSWORD   # 从 appleid.apple.com 生成
```

之后 `./scripts/sign.sh arm` 自动提交公证 + 装订。
