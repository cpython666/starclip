---
name: starclip
description: 星笺 Starclip 项目共享知识库。处理本仓库的功能开发、修复、设计、测试、文档或知识库维护前阅读，按主题查阅已确认决策、实现入口与验证边界；用户说“更新skill”时回顾当前整个对话并维护知识库。
---

# 星笺 Starclip

独立、本地优先的桌面剪贴板与代码收藏工作台，技术栈为 Rust / Tauri 2 / SQLite / Vue 3 / TypeScript / Vite。核心流程是复制、搜索、星标或分组，再进入星绘坊制作本地分享图片；无账户、云端后端或遥测。

## 关键约定

- 按根目录 [AGENTS.md](../../../AGENTS.md) 工作与维护知识库。先检查工作区、保护已有修改；文档更新不授予提交、推送或安装权限。
- 中文、星空主题、清晰用途说明。使用星码集、星绘坊、星码卡、月光笺、流星栏、星尘整理、星伴这些产品名称；第三方名称只在参考出处出现。
- 不用真实剪贴板、正式历史或个人数据做测试与截图，不将数据库、备份、日志或凭据入 Git。新安装为空且暂停记录，升级保留用户偏好。
- 区分源码实现、自动检查、浏览器演示、平台构建和原生实测。历史验收只证明对应版本与场景；当前范围以代码及 [README](../../../README.md)、[产品范围](../../../docs/product.md) 核对。

## 按需阅读

以下源码路径均相对仓库根目录；主题资料只在涉及该任务时加载。

| 任务 | 资料 |
| --- | --- |
| 找代码入口、改前后端协议或窗口权限 | [架构与实现入口](references/architecture.md) |
| 改界面、偏好、卡片或星伴，保留已确认体验 | [产品与交互约定](references/product-decisions.md) |
| 修改产品分享入口、文案或宣传图 | [分享素材说明](../../../docs/product-share.md)、[架构与实现入口](references/architecture.md) |
| 自定义主题色、渐变方向或本地 PNG 贴纸 | [自定义外观](../../../docs/appearance.md)、[产品与交互约定](references/product-decisions.md) |
| 数据开放、隐私规则、域名筛选或 AI 接入 | [隐私与来源规则](../../../docs/privacy-ai.md)、[AI 接入说明](../../../docs/ai-access.md)、[数据与隐私边界](references/data-safety.md)；[早期设计与待办](../../../docs/open-data-ai-plan.md) 不等于完整实现 |
| 规划星伴养成、检查贴纸来源与授权边界 | [养成规划与素材核对](references/growth-and-artwork.md) |
| 改采集、图片、保留策略、备份或迁移 | [数据与隐私边界](references/data-safety.md) |
| 实施升级、恢复与签名发布 | [数据与隐私边界](references/data-safety.md)、[开发验证](references/development.md)、[发布指南](../../../docs/releases.md) |
| GitHub Release 发布后自动同步 Homebrew | [Homebrew 自动化](../../../docs/homebrew-release.md)、[开发验证](references/development.md) |
| 选检查命令、做桌面验收、理解踩坑与待办 | [开发验证与已知限制](references/development.md) |

完整资料：[产品范围](../../../docs/product.md)、[星尘整理](../../../docs/cleanup.md)、[星伴](../../../docs/starpet.md)、[星芽设计](../../../docs/starpet-design.md)、[验证记录](../../../docs/verification.md)、[参考体验对照](../../../docs/reference-comparison.md)。
