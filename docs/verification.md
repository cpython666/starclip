# 0.1.0 首版验证（2026-10-24）

## 已完成

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
