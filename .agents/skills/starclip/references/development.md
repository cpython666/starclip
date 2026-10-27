# 开发验证、踩坑与待办

## 环境与检查

运行内存不能只量 `starclip` 主进程，也不能把系统所有 WebKit 相加。macOS 可用 `launchctl print pid/<pid>` 的应用域 / bundle ID 核实辅助进程归属，再用 `footprint --noCategories` 汇总；与 RSS、虚拟地址空间及安装包大小分开。2026-10-26 正式安装包标识版本 0.1.0、运行约 4 小时的只读快照约 398 MiB（主程序约 38 MiB），不是最新源码基准。随后隔离窗口释放验收：空库暂停、无星伴、关闭主窗由旧版 103.6 MiB 降为 54.1 MiB，下一次采样约 33.9 MiB；做过编辑后收起所有窗口约 47.4 MiB，AutoFill helper 仍在。主窗打开仍约 104 MiB，启用星伴约 199–230 MiB，不能承诺所有状态几十 MiB。条件与边界见 [验证记录](../../../../docs/verification.md)。

依据 `package.json`、`rust-toolchain.toml`、`src-tauri/Cargo.toml` 和 `.github/workflows/verify.yml`：Node.js 22.12+，Rust 1.90.0，Tauri 2。浏览器开发命令 `npm run dev` 固定在本机 1420 端口；桌面 `npm run tauri dev` 会启动原生应用，不能当作纯编译检查。

有行为变化时执行相关前端和 Rust 基础检查：

```sh
npm test
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml --no-default-features
```

Rust 修改检查格式；桌面改动还要检查 desktop 特性和平台打包。权限或窗口相关改动运行 desktop lib 测试（其中含生成后的运行时 ACL 验证，不创建应用或访问真实剪贴板）：

```sh
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --locked --manifest-path src-tauri/Cargo.toml
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
```

平台打包用 `npm run tauri build`；macOS 可用 `npm run tauri -- build --bundles app,dmg`。首次缺少生成图标时用 `npm run tauri icon public/starclip.svg`。CI 配置为 Apple Silicon macOS app/DMG 与 Windows x64 NSIS；是否成功须查对应运行，不能仅凭 workflow 推断。磁盘不足用 GitHub Actions 产物，不清理其他项目或个人文件。

仅修改文档/skill 时核对事实、链接、结构和 `git diff --check`，不把未运行的构建/实测写为本轮通过。测试数量和包大小是历史构建快照，不作为恒定项目属性。

## 回归入口

| 修改主题 | 优先查阅的测试 |
| --- | --- |
| 主题、颜色、偏好兼容 | `tests/theme.test.ts`、`tests/color.test.ts`、`tests/api-import.test.ts`、`store.rs` 内测试 |
| 自定义配色与本地贴纸 | `tests/appearance.test.ts`、`tests/appearance-panel.test.ts`，Rust `appearance.rs` / `store.rs` / ACL；原生使用原创 PNG、隔离应用标识 |
| 导出反馈、监听释放 | `tests/backup-export.test.ts` |
| 清理、图片、备份、迁移 | `tests/cleanup.test.ts`、`tests/image-clip.test.ts`、`tests/api-import.test.ts`，Rust `store.rs` / `images.rs` / `storage.rs` / `capture.rs` |
| 卡片排版、手写异步结果 | `tests/code-card.test.ts`、`tests/snippets.test.ts`、`tests/handwriting-preview.test.ts`、Rust `handwriting.rs` |
| 隐私、来源、标签与历史扫描 | `tests/privacy.test.ts`、`tests/privacy-panels.test.ts`、v3 备份夹具；Rust `privacy.rs` / `store.rs` / `storage.rs`，使用原创或公开 BIP-39 测试向量 |
| AI 授权与官方接口 | `tests/ai-api.test.ts`、`tests/privacy-panels.test.ts`；Rust `ai_access.rs`、`ai_bridge.rs`、`src-tauri/tests/ai_stdio.rs`，检查真实主程序 CLI / MCP stdio 与权限一致性 |
| 产品分享 | `tests/product-share-api.test.ts`、`tests/product-share-panel.test.ts`，Rust `share.rs` / `desktop.rs` / ACL；资产元数据必须与 `public/share/*.png` 一致 |
| 更新、恢复、版本发布 | `tests/update*.test.ts`、`tests/release.test.ts`，Rust `updater.rs` / `storage.rs` / `store.rs` / ACL |
| 星伴表情、大小与互动 | `tests/starpet*.test.ts`、`src-tauri/src/lib.rs` / `store.rs` / `desktop.rs` |
| 主窗安全释放、重建与草稿保护 | `tests/window-release.test.ts` 运行真实 App setup / IPC，`desktop.rs` 请求编号回归；原生核实 WebContent 退出及草稿保留 |

互动菜单回归集中在 `tests/starpet-desktop-menu.test.ts`：真实 Vue 组件的内存渲染、外部点击 / 焦点 / 显隐、触发器切换、延迟动作、监听释放，以及原生失焦事件的窗口范围。该测试不访问系统剪贴板，也不等于跨应用原生点击实测。

隐私面板回归验证规则保存失败保留草稿、预览后显式扫描，AI 面板验证排除隐私 / 待扫 / 图片、明确 ID 与字段、重复提交、失败和数据变化使预览失效。`api.ts` 新增运行时导入时，同步更新以 data URL 转译加载该文件的既有测试映射；浏览器内存模拟不能替代 Rust 检测或原生授权验收。

既有手写 CLI 实测测试默认忽略，需要本机独立渲染器；核心测试通过不代表已完成手写集成验收。不要为凑数运行真实应用或读取剪贴板。

## 已踩过的坑

- 同日窗口优化检查为 Node 84、Rust core 73、desktop lib 82 通过（Rust 各 1 项忽略），Apple Silicon 独立 app 本地验签通过；有效原生场景见验证记录。首次关闭后不能立即用 CUA 读取应用来测后台：读取可能激活 / 重开主窗口，应先以进程工具采样。去滤镜 A/B 无确认收益已恢复；多副本实验意外开启了其中一个隔离副本采集，停止并恢复暂停，未查看新增图片，该实验作废。后续一次只运行一个验收副本，并核对暂停状态；不能把异常实验写成未访问剪贴板的通过项。
- 原生 dialog 顶层会遮住外部 toast：导出结果放在设置内，通用提示归当前活动 dialog。整理弹窗不能替换/销毁父设置，Escape 与保存快捷键要遵循当前层级。
- 即时主题和星伴更新不能调用普通偏好保存，否则会带入草稿或清理副作用；快速连续写入需顺序合并、失败恢复最后确认值，跨窗口采用补丁合并。
- 独立保存的 checkbox / number 原生 DOM 可能先改变而 Vue 状态仍为旧值；失败回退需同步 DOM，不能只依赖「赋回相同 ref」。外观组件已覆盖。自定义色需要同时考虑强调色文字、按钮前景和受渐变影响的阅读底色；删除覆盖变量才能完整恢复主题默认。
- macOS 原生拖动要在按下时直接发起，等待移动阈值/长按或异步加载会错过起拖时机；命令成功或入口存在仍不等于实际位移验收。
- 原生截图可能被工具扁平为 RGB 白底，不能证明透明窗口真实透显；数值运动测试不能证明流畅度或长期 CPU 占用。
- macOS Dock 重新打开需处理 `RunEvent::Reopen`；`has_visible_windows` 包含星伴浮窗，不能据此跳过主窗显示。2026-10-26 已接入现有 `show_window`。CUA 本轮无法读取 Dock 控件，且关闭后读目标状态无法可靠区分工具激活，真实 Dock 点击 / 最小化 / 仅星伴可见场景仍待人工确认；隔离原生默认不限条数和天数已核对。依据见当日验证记录。
- 同轮增强既有不限历史测试：550 条原创记录全部设为很早的更新时间后，默认清理仍删除 0 条。Node 58 / Rust core 54 / desktop lib 58 项通过（Rust 各 1 项忽略），Apple Silicon 打包与 app / updater 签名核验通过；本地产物 `artifacts/dock-retention-0.2.0/` 未安装到正式应用。以上只证明记录中的构建和场景，不替代真实 Dock 点按验收。
- 2026-10-26 默认 Tauri 打包的整包签名核验出现「code has no resources but signature indicates they must be present」。通过 `tauri bundle --bundles app,dmg --config '{"bundle":{"macOS":{"signingIdentity":"-"}}}'` 重新做本地 ad-hoc 签名，两处 app（产物及 DMG 内）通过 strict/deep 核验；不等于公证。DMG 验签挂载点放在开发服务器监听目录之外，或先停 Vite，避免其沿安装器的 Applications 链接扫描其他应用资源。
- 不把编译、浏览器演示或某次 macOS 场景通过扩大为 Windows、多屏、全屏或全功能验收。历史安装包、本地产物与早期 alpha.1 发布附件可能不同，交付时核对对象。

## 已有证据与未验收范围

隐私与 AI 首版原生边界（2026-10-26）：独立 `com.starclip.privacy-verify` 导入 8 条原创 / 公开向量，2 条被凭据与助记词检测保护并排除授权；真实创建 / 撤销 2 条普通记录的连接，配置未写日志。最终包重启后保持 8 条记录和暂停采集，AI 恢复默认关闭；app 本地验签与许可资源核对通过，未安装正式版。完整记录见 [验证记录](../../../../docs/verification.md)，Windows 与真实第三方 AI 客户端仍待验收。

截至 2026-10-26，仓库 [验证记录](../../../../docs/verification.md) 已记录 macOS 隔离验收的清理保护、迁移/恢复、首启教程、主题控件、图片采集/复制与关闭主窗继续采集、星伴改色/大小/重启恢复、导出路径及清理返回设置。它们是历史证据，不表示每次修改后都重新执行了这些原生验收。

本轮隐私 / AI 自动检查：Node 76、Rust core 73、desktop lib 78 通过（Rust 各另 1 项忽略），真实主程序 CLI / MCP stdio 集成 1 项通过；前端生产构建通过。测试覆盖隐私不降级、v3 导入停用规则、扫描预览过期、字段 / ID / 记录身份授权、撤销、冻结与并发读取。此处不预写正在进行的原生界面验收，最终平台与客户端证据以验证记录为准。

养成第一版本轮新增证据：34 项 Node、41 项 Rust core、43 项 desktop lib 通过（Rust 各另有一项忽略）；隔离原生界面验证 45 XP / Lv.2 / 眼镜、重启恢复、暂停不加经验、独立存档取消 / 导出 / 确认导入，以及成功保存原创代码从 25 XP 变为 45 XP。隔离库最终一条手动测试记录，始终暂停采集；浏览器验证修订冲突与 70% / 160% 装扮。详情与版本边界仍以验证记录为准。

同日用户随后明确要求安装：将已核验养成 app 替换到 `/Applications/Starclip.app` 并启动。操作前在仓库外私有目录保留完整 SQLite 一致快照、受管文件与旧应用；启动后仅比较本机摘要，clips / clip_images / groups / preferences、数据定位、手写配置与 renderer 一致，无真实正文输出。本次备份是人工升级操作的一部分，不是应用已有自动备份能力。

## 产品分享验证边界

分享入口为设置 → 帮助 → 分享星笺；源码 `ProductShare.vue`、`product-share.ts`、`share.rs` 已实现固定文案 / 图片复制及专用保存命令。定位改版后三张 1080 × 1440 索引 PNG 合计 248,512 字节（约 243 KiB）；素材源、重制和压缩流程见 [分享说明](../../../../docs/product-share.md)。Python / Pillow 仅用于制作素材，不增加运行或正常构建依赖。历史包大小与本轮素材不同，验收按验证记录分轮查看。

Rust 纯目录 / PNG 校验、模拟成功 / 失败写入的自采集抑制、生成后的窗口 ACL 已检查；浏览器复制测试采用模拟接口，不读取真实剪贴板。不要把模拟复制成功写成系统剪贴板 / 第三方粘贴实测。2026-10-26 独立 `com.starclip.share-verify` 原生实测保存实际路径与取消，保存 PNG 摘要与源文件一致；Node 58 / Rust core 54 / desktop lib 58 项通过（Rust 各 1 项忽略）。最终分享版 app / DMG / 签名 updater 归档位于忽略的 `artifacts/product-share-0.2.0/`，本地 app / DMG 内应用及更新签名核验通过，未替换正式安装。具体结果与边界由 [验证记录](../../../../docs/verification.md) 维护，不沿用此前升级版验收。

Tauri 资产解析器在正式包中读取嵌入资产，在带 `devUrl` 的开发模式回退到 `dist/`；修改 PNG / catalog 后先跑 `npm run build`，否则旧尺寸或字节数会导致校验失败。PNG 被复制为系统图片像素，接收方可能重编码变大；实际小文件分发使用「保存图片」。

## 免费安装包分发候选（2026-10-26 核对）

- 用户后续已要求开通 Homebrew 分发，使用既有 GitHub 账号建立公开 [cpython666/homebrew-tap](https://github.com/cpython666/homebrew-tap)，无需 Homebrew 账号。`Casks/starclip.rb` 提供 Apple Silicon 的 `0.1.0-alpha.2`，命令 `brew install --cask cpython666/tap/starclip`；不是 Homebrew 官方主仓收录。其它渠道仍待定。0.2.0 应用内更新源仍是 GitHub Releases（`updater.rs` 限定下载 URL、`release-manifest.mjs` 生成清单）；SourceForge / Codeberg 没有更新回退接入。详见 [发布指南](../../../../docs/releases.md)。
- [GitHub Releases 官方说明](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases)：可分发二进制，每个附件小于 2 GiB，每个 release 最多 1,000 个附件，未设 release 总大小与下载带宽额度上限。公开开源项目可使用免费方案；安装包应放 release 附件，不写进 Git 历史。
- [SourceForge 官方项目页](https://sourceforge.net/create/)：面向开源项目提供免费全球下载镜像和不限下载带宽，可分发现有软件版本，不要求迁移主代码仓库。可作为备用手动下载候选；国内可达性 / 速度未实测。GitHub 与 SourceForge 的免费服务不等于非营利组织运营。
- [Codeberg 官方 FAQ](https://docs.codeberg.org/getting-started/faq/)：由非营利 Codeberg e.V. 支持，服务自由开源项目；[默认额度说明](https://blog.codeberg.org/new-storage-limits-on-codeberg-what-you-need-to-know.html) 为每用户 / 组织 750 MiB Git 存储，以及额外共享 1.5 GiB LFS / Packages / Releases / Attachments，合理开源需求可申请增加。不可表述为无条件无限安装包托管。
- 命令安装渠道与二进制托管分开：[Homebrew 自建 Tap](https://docs.brew.sh/How-to-Create-and-Maintain-a-Tap) 存 Cask 等安装描述，星笺已采用；[WinGet 社区仓库](https://learn.microsoft.com/en-us/windows/package-manager/package/repository) 收 YAML 清单并验证 / 审核，安装文件仍需上游 URL，星笺尚未上架。[PyPI 的 wheel / 源码包格式](https://packaging.python.org/en/latest/discussions/package-formats/) 面向 Python 包，不是任意 DMG / EXE 的附件托管；未来 Python SDK / CLI 属另一项待定需求。
- Homebrew 预览包沿用 alpha.1 原源码提交 `1fd865372ac66c66f007a30845dc18a740ac5c2d`，只修复其不完整的应用资源签名并重封装；发布为独立 `v0.1.0-alpha.2` prerelease，保留旧附件、不设 latest、不提供稳定更新清单，内部应用版本仍为 0.1.0。不能表述为本地 0.2.0 新功能已上线；本地 ad-hoc 验签不等于 Apple 公证。Cask 无 `zap` / 删除数据脚本、无 `auto_updates`，预览版 livecheck 明确手动维护。验证证据见 [验证记录](../../../../docs/verification.md)。
- Homebrew 7.0.9 要求 Cask 位于已添加的 Tap，再运行 style / audit；本轮公开下载、校验、独立 `--appdir` 安装、签名与普通卸载通过，未启动测试副本 / 触碰正式数据。最终 online audit 仅有 GitHub prerelease 报告，不能写成全部通过；不要使用已禁用的旧 `--no-signing` 参数。工具自动开启 developer 模式时，验收后恢复原状态。
- 2026-10-27 用户要求发布后自动同步 Cask，新增独立 `homebrew.yml` / `homebrew-release.mjs`：只接受公开 latest 稳定 `vX.Y.Z`，验证实际 DMG 的大小 / SHA-256 / GitHub digest、只读挂载及签名 / 版本 / 标识 / 架构，再跑 Homebrew style / audit / fetch；最后单文件非强推 Tap。共享队列之外仍做数字版本比较、同版换包拒绝、推送前资产快照复查。人工 Publish 可触发；未来通过 `GITHUB_TOKEN` 自动发布时必须显式 dispatch，不会自动产生下游 release 事件。见 [同步说明](../../../../docs/homebrew-release.md)。
- 本地 10 项同步脚本回归、完整工作区 Node 94 项、Web 构建、Rust core 73 项（1 项忽略）和 actionlint 通过；在独立临时 Tap 中实际 `brew style --cask` 通过，未安装 / 启动任何包。Homebrew 7.0.9 将 Big Sur 最低版本视为冗余约束，模板沿用 `depends_on :macos`；直接以普通 Ruby 文件检查会套用错误的 frozen-string 规则，应在 Tap 上下文检查。临时 Tap 已移除，自动开启的 developer 模式已恢复关闭。
- 只写 `cpython666/homebrew-tap` 的专用 deploy key 及主仓 `HOMEBREW_TAP_DEPLOY_KEY` secret 已配置，并通过不改远端 ref 的 SSH push dry run；没有复用本机登录令牌或 Tauri 更新密钥。真实新稳定版 DMG 的在线同步仍须下一次合格 Release 验收；预发布跳过不是完整链路通过。
- [PR #1](https://github.com/cpython666/starclip/pull/1) 已于本地时间 2026-10-27 合并到远端 `main`（`fe40fe7`），只部署 Homebrew workflow、helper、版本 helper、10 项测试和独立使用说明；head `22181c0` 的 [CI](https://github.com/cpython666/starclip/actions/runs/40356060862) core / macOS / Windows 全部通过。原工作区其余 0.2.0 功能、签名草稿发布流程未随该 PR 提交；不能将同步自动化上线称为 0.2.0 软件已发布。
- 合并后的 [alpha.2 dry run](https://github.com/cpython666/starclip/actions/runs/40357536518) 成功输出预发布跳过，Tap job 未运行，Tap main 仍为 `e11f8f0`；仅证明触发 / API / skip 路径。首次公开稳定版的 DMG 在线校验和 Cask 推送仍待验收。

## 0.2.0 升级机制

- 程序和数据独立，保持 `com.starclip.desktop` 标识即可沿用默认目录及 `data-location.json` 指定目录。0.1.0 没有更新器，首次升级需退出、手动覆盖程序并重启；不清理数据目录。启动仍执行用户保存的保留规则。
- 用户已要求实现四项升级能力及下载后确认安装体验；源码现有官方 `tauri-plugin-updater`、固定 GitHub latest 端点、公钥和 `requireSignedVersion`。`download_starclip_update` 只下载验签，`ready` 缓存同会话包、不冻结数据；原生 `<dialog>` 提示「安装并重启」或稍后，ESC / X / 遮罩不关闭父设置，同 ready 状态不反复弹。`install_starclip_update` 仅接受已下载且确认版本一致的包，再做快照 / 旧应用备份 / 安装；macOS 成功后前端调用重启，Windows 显式 `restart_after_install(true)`。重启失败保留冻结与重试。没有启动自动联网；仅主窗能调用包装命令，不开放插件直接安装 IPC。入口：`updater.rs`、`desktop.rs`、`UpdateSettings.vue`、`update-api.ts`。
- 两阶段下载缓存只在内存中，重新检查、失败或退出会清除，没有磁盘缓存 / 断点续传；不能称为重启后免下载。安装确认前需保存偏好，旧确认遇新版本必须失效；下载事件与调用响应按 revision 去重。验证边界以 [验证记录](../../../../docs/verification.md) 为准。
- 两阶段更新本轮验证（2026-10-26）：Node 80、Rust core 73、desktop lib 81 通过（Rust 各 1 项忽略），前端构建与 Apple Silicon 独立 `com.starclip.update-verify` release app / 本地验签通过。真实 Vue 浏览器固定 IPC 夹具验证弹窗、稍后、草稿保护和重启失败；不是线上更新或 Windows 实机验收，未安装正式版 / 发布 0.2.0。
- 升级机制引入显式结构版本；当前隐私元数据迁移已升至 `user_version=2`，legacy / 旧结构先一致快照再事务迁移，未来结构只读拒绝。恢复点带摘要并存入默认应用数据目录 `recovery/`；恢复前备份当前库、复制到新目录、更新定位并冻结写入等待重启。旧应用副本另存 `application-backups/`；不是自动二进制降级，0.1.0 本身没有新结构拒绝保护。详见数据安全资料。
- 当前源码版本已统一递增至 0.2.1（2026-10-27 发布准备），`scripts/version.mjs` 校验 / bump npm、Cargo、Tauri 六处值；`CHANGELOG.md` 提取发布说明。`release.yml` 校验已有 tag、两平台签名构建、minisign 核验后只上传完整草稿，不自动公开；`verify.yml` 无私钥也能检查和打包。macOS runner 已改为 `macos-15`，两个构建流程增加 desktop lib 与 CLI / MCP 测试；新流水线尚待上传和线上验证。完整步骤见 [发布指南](../../../../docs/releases.md)。
- 本轮 Node 53 / Rust core 52 / desktop lib 55 项通过（Rust 各 1 项忽略）；macOS 独立标识实测 legacy 迁移、手动快照、修改后恢复到新目录和重启，原目录保留，暂停偏好保持。正式用户数据未用于测试。发布脚本另有真实临时文件签名 / 篡改拒绝核验，两 workflow 的 actionlint 通过。细节以验证记录为准。
- 最终 Apple Silicon 0.2.0 app / DMG / 签名 updater 归档已构建，app 与 DMG 内 app 的本地 strict/deep 验签及真实更新包 minisign 公钥 / 签名版本 0.2.0 核验通过；产物在忽略的 `artifacts/upgrade-0.2.0/`。本轮未安装到用户正式应用。
- Tauri 更新私钥仍保留在仓库外私有目录，未上传 GitHub；该升级实现阶段未推送或公开 0.2.0。后续 Homebrew 自动化配置的是独立 Tap deploy key secret，不代表已配置 Tauri 签名发布。源码与本地验签不能替代公开更新源及 Windows/macOS 完整升级验收。
- 0.2.1 准备阶段已用临时原创文件及标准 Ed25519 校验确认原密钥匹配内置公钥、签名绑定版本且拒绝篡改。Node 97、Rust core 78、desktop 88、CLI / MCP 1 项及 Web / Mac 编译通过；本地 release app、只读挂载 DMG 内应用和 updater 的签名 / 版本已核验，候选保存在忽略的 `artifacts/release-0.2.1-macos/`，未安装或启动，不替代原生升级验收。GitHub TLS 连接失败暂时阻止远端上传，签名 secret、双平台线上构建、正式 Release 与首次稳定 Cask 同步均未完成；没有修改用户代理。见验证记录当日条目。
- Windows updater 在启动安装器之前执行退出清理；安装器若被系统拒绝，必须保持数据冻结并进入 `restart-required`，不可当普通下载失败解冻。安装器路径尚未在 Windows 实机验证。源码依据：`desktop.rs` 的 `on_before_exit` 标记和失败分支。

仍待验收：Windows 安装与原生剪贴板/托盘/迁移/手写流程；星伴精确拖动、真实透显、多屏及跨全屏；托盘菜单实际点击；快捷键冲突、休眠恢复、长期资源占用和大量历史性能。各项以 [星伴说明](../../../../docs/starpet.md) 和验证记录中的具体证据为准。

已知未实现/后续方向：文件历史、GIF 动画、富文本、OCR、备注、多片段集合、自动粘贴、云同步/账号/剪贴内容公开链接、代码执行与内置 AI 分析（已有只读外部 AI 接口不等于模型分析）、密码管理器标记识别/来源应用排除、手写组件随包分发。它们不代表排期或已承诺实现；新讨论中的方案标「待定」。分发仍为开发预览版，无 Apple Developer 公证或 Windows 代码签名，本地 ad-hoc 校验不等于正式签名。
