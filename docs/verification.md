# 0.1.0 验证记录（2026-10-24）

## 卡片工坊更新

- `npm test`：4 项通过；新增中文、CRLF、留白和代码卡片容量边界检查。`npm run build` 通过。
- Rust 无 desktop 特性测试：9 项通过，1 项本机 CLI 测试默认忽略；格式检查与桌面 `cargo check --locked` 通过。
- 显式运行 `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features local_cli_renders_original_text_to_png -- --ignored` 通过；真实星笺 CLI 生成 1200×1600 纯色纸及 200×200 内置纸，验证纸张尺寸与临时文件清理。
- 独立应用标识 `com.starclip.card-verify` 的原生构建实测：发现本机星笺及完整 9 款字体，输入原创中文 → 生成预览 → 原生保存对话框 → 成功保存 1200×1600 PNG。
- 原生修改留白后显示预览失效提示并禁用导出；更新预览后恢复。手写模式 `⌘S` 未增加代码记录，退出工坊后仍为 0 条记录、暂停采集；未读取或改动正式应用历史。
- 浏览器演示实测：代码/手写切换保留正文；已有文字片段直接进入手写模式；浏览器明确提示手写需要桌面版。代码卡片实际下载为 1184×548 PNG，中文、首行空行、紫色背景、56 px 横向留白和自定义水印均正确。
- 两种 PNG 均打开检查了实际图片。验收仅使用原创示例；临时浏览器预览和独立验收应用已退出。
- 正式应用标识的本地打包成功，Apple Silicon `.app` 约 8.20 MiB，`.dmg` 约 3.57 MiB；未签名公证。
- 手写能力当前依赖单独安装的星笺 Rust 应用；没有把字体及渲染器捆绑进 Starclip。Windows 手写接入尚未实机验证。

![原生卡片工坊：手写纸笺预览与导出成功](card-studio.png)

## 首版已完成

- Apple M3 Pro 本地 `npm run build`：TypeScript 检查及 Vite 生产构建成功。
- `npm test`：3 项通过，覆盖 Markdown 围栏逃逸、导出元数据与 JSON 格式化。
- `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features`：6 项通过，覆盖去重与分组删除、代码编辑冲突、历史容量、备份合并与原子拒绝、Unicode / 大小限制、备份条数与分组数上限往返。
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`：通过。
- 本地 `npm run tauri build -- --bundles app,dmg`：成功产出 Apple Silicon `.app`（约 8.04 MiB）和 `.dmg`（约 3.49 MiB）。
- 原生应用成功启动；初次为空且暂停。通过界面保存原创 Rust 片段，退出重启后仍可读取，验证了前端 → Rust → SQLite → 重启读取链路。
- 短时核实系统文本变动可进入原生历史；验证期间的临时捕获内容已清理，未加入源码或截图。仅保留原创「你好，星笺」示例。
- 浏览器交互检查：搜索有结果 / 无结果、分组创建、新建片段、代码保存、JSON 格式化、深浅主题。
- 在实际界面下载 PNG 和 Markdown，检查真实文件内容；浏览器 JSON 备份已核对 `format/version/exportedAt/clips/groups` 兼容结构。
- [GitHub Actions 验证运行](https://github.com/cpython666/starclip/actions/runs/39999168843) 中，核心测试与 macOS 构建通过。Windows 状态以同次运行记录为准。

![由代码编辑器真实导出的 PNG 卡片](code-card.png)

## 尚未验证或未覆盖

- Windows 安装程序运行、托盘与剪贴板的实机体验；没有操作远程 Windows 电脑。
- 其他应用占用快捷键、休眠恢复、长期运行与大量历史的性能。
- 密码管理器剪贴板标记、来源应用排除：尚未实现。
- 没有 Apple Developer 公证或 Windows 代码签名；分发包为开发预览版。

图片仅包含原创示例，没有发布真实剪贴历史、数据库、账号或个人文件。
