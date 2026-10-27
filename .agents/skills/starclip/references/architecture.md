# 架构与实现入口

以下路径相对仓库根目录。依据当前源码核对；这里只记录定位与耦合关系，不复制实现。

## 前端

| 路径 | 职责与修改注意点 |
| --- | --- |
| `src/main.ts`、`src/App.vue`、`src/starpet-shell.css` | 入口按 `?view=starpet` 动态加载桌面星伴或主应用；星伴使用独立外壳，不加载主应用全局样式。主应用管理列表、详情、设置草稿、弹窗和快捷键 |
| `src/types.ts`、`src/api.ts` | 前后端数据形状与调用入口；用 `__TAURI_INTERNALS__` 区分桌面 invoke 和原创内存演示。修改行为时检查两条分支及 Rust 类型 |
| `src/components/ProductShare.vue`、`src/product-share.ts`、`src/product-share-catalog.json` | 帮助页分享入口、两套文案与三张固定产品 PNG、复制 / 保存回执；元数据与 Rust 共用 |
| `src/components/UpdateSettings.vue`、`src/update-api.ts`、`src/update-state.ts` | 更新与恢复界面、进度订阅、确认版本固定、恢复后重启保护；浏览器仅说明 |
| `src/privacy.ts`、`src/components/PrivacyRules.vue`、`src/components/AiSettings.vue` | 标签 / 来源 / 隐私筛选、规则与重扫、本地 AI 授权及连接配置；`src/api.ts` 包装命令，浏览器只作固定数据演示 |
| `src/components/DataSettings.vue`、`src/components/CleanupPanel.vue` | 数据位置、备份回执与目录操作；整理预览和确认。父设置的挂载状态由 `src/App.vue` 保留 |
| `src/components/CodeStudio.vue`、`src/code-card.ts`、`src/snippets.ts` | 星绘坊在打开时按需加载；Canvas 代码卡片、语言与导出工具，卡片预览和 PNG 共用渲染 |
| `src/components/HandwritingPanel.vue`、`src/handwriting-preview.ts` | 本地手写预览调度、过期结果隔离与导出有效性；`src/api.ts` 串行化手写资源发现、选择与渲染 |
| `src/components/ClipImage.vue` | 可见区加载图片，详情原图与缩略图分开取用 |
| `src/theme.ts`、`src/color.ts`、`src/styles.css` | 即时主题保存、扩散效果、颜色规范及全局样式 |
| `src/appearance.ts`、`src/appearance-api.ts`、`src/components/AppearanceSettings.vue` | 独立强调色 / 渐变 / 本地 PNG 外观、可读性调整、失败回退；`StickerScene.vue` 显示自定义图 |
| `src/components/UiSelect.vue`、`src/components/UiColor.vue` | 统一下拉和颜色控件；保留字符串、数字、null 值语义、键盘与合法 HEX 校验 |
| `src/components/StartupScreen.vue`、`src/components/OnboardingGuide.vue`、`src/components/StickerScene.vue` | 流星启动画面、四步教程和原创贴纸 |
| `src/components/StarPet.vue`、`src/components/StarPetSettings.vue`、`src/components/StarPetDesktop.vue` | 共享 SVG 角色、设置预览、原生浮窗 UI |
| `src/starpet.ts`、`src/starpet-api.ts`、`src/starpet-size.ts`、`src/starpet-motion.ts`、`src/starpet-behavior.ts`、`src/starpet-action.ts` | 表情定义、独立偏好通信、大小保存队列、运动、情境互动与主动动作姿态 |
| `src/starpet-growth-catalog.json`、`src/starpet-growth.ts`、`src/starpet-growth-api.ts`、`src/components/StarPetGrowth.vue` | 前后端共用任务 / 动作 / 物品 / 成就目录；成长类型与存档校验、浏览器内存演示、专用命令 / 事件和成长设置页。预览不记奖，窗口按修订号接收状态 |
| `src/starpet-signal-api.ts`、`src/components/StarPetDesktop.vue` | Agent 临时状态 / 动作 / 短句；监听 `starpet-signal` 后读取 `get_starpet_signal` 快照，revision 防迟到覆盖，TTL与卸载清理，不走成长奖励命令 |

## Rust 与桌面

| 路径 | 职责与修改注意点 |
| --- | --- |
| `src-tauri/src/lib.rs`、`src-tauri/src/main.rs`、`src-tauri/Cargo.toml` | 默认启用 `desktop`；主程序在 Tauri 启动前分流 `ai` / `mcp` 模式。无默认特性可测存储与纯逻辑，不启动桌面 |
| `src-tauri/src/desktop.rs` | 命令注册、共享状态、采集循环、系统剪贴板、托盘、全局快捷键、原生窗口与重启 |
| `src-tauri/src/updater.rs` | 原生更新状态与互斥、正式下载源限制、旧应用备份；desktop.rs 包装检查 / 下载 / 备份 / 安装命令 |
| `src-tauri/src/capture.rs` | 开启时捕获当前内容、变更和重试状态机；本模块不访问真实剪贴板 |
| `src-tauri/src/store.rs` | schema 2 升级、记录 / 分组、隐私元数据与随机记录身份、规则 / 重扫、保留清理和 JSON v3 备份事务；所有内容写入在同事务扫描 |
| `src-tauri/src/privacy.rs`、`src-tauri/resources/bip39/` | 本地敏感检测、URL 解析与来源、关键词 / 精确域名规则、手动和自动标签区分；词表来源许可随资源保存 |
| `src-tauri/src/ai_access.rs`、`src-tauri/src/ai_transport.rs` | 会话连接、固定 ID + 随机身份授权、统一读取过滤、独立星伴权限与限时内存信号、审计及有界消息；stdio MCP / CLI 经本机桥接，不能直接打开数据库 |
| `src-tauri/tests/ai_stdio.rs` | 启动实际主程序子进程，验证 CLI 与 MCP stdin/stdout，不读取正式用户数据 |
| `src-tauri/src/starpet_growth.rs` | 成长、日限额、解锁、装备与存档校验；`store.rs` 独立事务存储，`desktop.rs` 在代码保存 / 导出成功后授奖并广播 |
| `src-tauri/src/share.rs` | 固定分享 ID、路径 / PNG 字节与尺寸校验；desktop.rs 使用已打包资产并负责复制 / 保存，禁止任意内容或路径参数 |
| `src-tauri/src/images.rs` | RGBA/PNG 校验、像素摘要、缩略图和尺寸限制 |
| `src-tauri/src/appearance.rs` | 自定义外观白名单补丁、静态 PNG 结构与有界解码、512px 缩图；`store.rs` 独立偏好事务，四命令仅主窗授权 |
| `src-tauri/src/storage.rs` | `data-location.json` 定位、数据目录校验、SQLite 一致快照、迁移与采用已有目录 |
| `src-tauri/src/handwriting.rs` | 发现/选择独立手写应用、读取资源与本地 CLI 渲染；配置为 `handwriting-app.json` |

SQLite 主要表为 `clips`、`clip_images`、`clip_privacy`、`groups`、`preferences`。`clip_privacy` 的 JSON 元数据保存手动隐私、持续保护原因、规则扫描版本、手动 / 自动标签、域名来源和随机记录身份。偏好独立使用 `settings`、`onboarding_v1`、`starpet_v1`、`starpet_growth_v1`、`appearance_v1`、`privacy_rules_v1`；教程、主题、外观、成长与规则保存不能随意换成完整设置保存，后者带有清理副作用。JSON v3 含记录、手动标签、隐私与规则，不包含其它偏好；成长另有独立存档。

`desktop.rs` 中本机桥接只绑定回环临时端口，按存储冻结 → 记录 → AI 授权锁序读取并持锁至响应写出。CLI / MCP 在主应用未开启接入时不可读；`get_state` 仍为主窗全量接口，不能用作 AI 数据出口。接口用法和边界见 [AI 接入](../../../../docs/ai-access.md)、[隐私与规则](../../../../docs/privacy-ai.md)。

## 窗口权限

- `src-tauri/tauri.conf.json` 配置主窗口 `main` 与 CSP；星伴窗口 `starpet` 由 Rust 创建。
- `desktop.rs` 收到主窗关闭先隐藏，再向 `App.vue` 发送带令牌的关闭请求；前端经 `api.hideWindow(release, requestId)` 确认安全空闲后销毁 WebView。编辑、设置、星绘坊、教程、进行中任务及尚未初始化时保留窗口；后端另拦截更新进行中、待重启与存储冻结。重新打开先作废旧令牌，在工作线程等待销毁完成并按配置重建，后台采集与托盘继续；只缓存本次运行的尺寸、位置和最大化状态，重新应用置顶，不持有 WebView。重建不重播启动画面，搜索 / 筛选 / 滚动等未持久化界面状态复位；Windows 此流程未实机验收。
- 星伴停用成功保存偏好后销毁窗口，重新开启时重建；失败保留既有偏好回滚处理，星伴位置不持久化。
- 自定义命令同时涉及 `src-tauri/build.rs`、`src-tauri/permissions/windows.json`、`src-tauri/capabilities/default.json`、`src-tauri/capabilities/starpet.json` 和 `src-tauri/src/desktop.rs`。增加命令时一起核对注册与最小授权。
- 隐私 / 标签 / 规则 / 重扫、AI 开关 / 授权 / 配置 / 审计命令仅主窗口可用；星伴不能借这些命令取得历史。AI 工具不允许改变权限或写入数据。
- 产品分享三个命令仅主窗口可用；读取固定打包素材，不授予星伴窗口剪贴板权限。剪贴写入与既有历史复制共用 helper，成功后才登记自采集抑制；保存使用专用命令，不走会授予成长经验的 `save_export`。
- 星伴有八个自定义命令：读取 / 修改自身偏好、打开主窗、原生拖动，以及读取成长、启停成长、宠物互动、装备选择；另有事件监听权限。成长导出 / 导入 / 重置三个命令仅主窗口可用，星伴不得读取历史、图片或剪贴板，也不能自行申报工作完成奖励。`src-tauri/src/lib.rs` 含声明与生成后运行时 ACL 测试。

依赖与构建入口见 `package.json`、`rust-toolchain.toml`、`.github/workflows/verify.yml`。现有 UI 直接使用 Vue 与 CSS；新能力优先复用现有组件。

版本与分发：`scripts/version.mjs`、`scripts/release-manifest.mjs`、`CHANGELOG.md`、`.github/workflows/{verify,release}.yml`；固定 endpoint 与公开验签密钥位于 `tauri.conf.json`，私钥不得入仓库。见 [发布指南](../../../../docs/releases.md)。

Homebrew 同步独立使用 `.github/workflows/homebrew.yml`、`scripts/homebrew-release.mjs` 与 `tests/homebrew-release.test.ts`。读取公开 latest 稳定 Release，校验实际 DMG 与清单及应用身份后生成 Cask；仅末尾 Tap job 使用单仓 deploy key，推送前重查 Release，拒绝降级 / 同版换摘要。详见 [Homebrew 自动化](../../../../docs/homebrew-release.md)，不能把同步验包称为完整安装升级验收。

产品宣传素材源为 `scripts/share-posters.html`，直接渲染既有星芽组件与原创界面 / 代码示意；`scripts/compress-share-images.py` 制作 256 色索引 PNG 并同步 catalog，Pillow 仅为素材制作依赖。见 [分享说明](../../../../docs/product-share.md)。
