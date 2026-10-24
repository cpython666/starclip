# 星芽：原创星星桌宠

状态：已实现圆润原创 SVG、32 种表情、独立改色与桌面轻互动；本轮原生改色同步、互动及重启恢复已验证，正式包已替换，正式启动与数据一致性核对通过。透明视觉等限制见下文。参考许可核对日期：2026-10-24。

## 当前功能

- 「偏好设置 → 星伴」提供原创星星角色「星芽」：先用 ImageGen 生成 [概念图](starpet-concept.png)，再据图绘制圆润 SVG；[设计说明](starpet-design.md) 保存完整提示词，参考位图只作设计资料、不进入安装包。日常陪伴 8 种、小小心情 12 种、一起做事 12 种，共 32 种状态保持不变。ID、轮廓、表情与动画参数由本项目独立设计，没有复用参考参数表或实现。
- 表情墙可点选预览、按分类筛选，也可开启或暂停巡游。巡游会切换页内主预览，不自动把每个表情应用到桌面。
- 桌面星伴默认关闭。点击「让星芽住在桌面」开启；选中其他表情后可点「用这个表情陪我」。开关、应用表情与颜色独立写入 `starpet_v1`，立即保存，不依赖「保存全部偏好」，不保存其他偏好草稿，也不触发历史清理。
- 配色提供「奶油星光」「蜜桃云朵」「薄荷心事」「薰衣草梦」「晴空来信」5 套预设，也可输入完整 HEX。改色即时保存，并通过事件同步到已开启的桌面浮窗；换心情继续保留颜色。旧版 `starpet_v1` 缺少 `color` 时读取为奶油金 `#FFD66E`，不会仅因补默认值而重写原记录。
- 桌面端配置为 230×260 的独立透明 Tauri 窗口，本体与顶部把手均可发起原生拖动，另有摸摸、换心情、打开星笺与收起入口。当前是手动轻互动，不是 AI 助手；不联网、不读取剪贴内容，也不分析正文。复用现有技术栈，没有引入大型运行依赖。
- 浏览器仅预览表情，不开启原生桌面浮窗。参考仅限眨眼、注视、呼吸和状态切换等一般表现，没有复制 Grok 角色、第三方素材、眼形坐标、关键帧或受限引擎代码。

## SVG 动作与互动

- 按住本体即可抓起，按下时也会给一次摸摸回应；左键直接发起系统拖动，不等待长按或移动阈值。鼠标 click 不重复触发，键盘 / 辅助激活保留摸摸。拖动没有新增权限，仍只允许移动星伴自己的窗口；位置暂不跨重启保存。

- 默认以无手的星星轮廓出现，32 种状态中 25 种不显示手；仅挥手、拿花、放大镜、读书、写字、画画、捧杯 7 种状态显示两颗半径 9 的小圆球。圆球按道具位置握住边缘，移除手指、手臂与手掌轮廓，沿用现有动作。

- 星角以 SVG 路径柔性变形，目光、脸部和身体分别使用弹簧运动，让转头有分层的跟随感。
- 眨眼采用不等间隔的单眨和双眨；表情切换带闭眼过渡。跳跃依次表现蓄力、起跳、落地压扁与回弹；腾空位置沿真正的二维三次贝塞尔曲线运动，高约 29 px、横向位移小于 8 px，缩放仍按阶段进行三次贝塞尔缓动。
- 设置页主预览可以点击，桌面每次摸摸都会传入新的互动信号，并出现 6 颗柔和星光。同一状态连续点击也能重播动作，保留当前姿态与速度，不从初始姿态硬切。
- 动画循环只在组件已挂载、`animated` 开启、系统未要求减少动态效果且页面可见时运行。停用时取消 `requestAnimationFrame` 并保持静态，卸载时清理循环；图库以静态缩略图为主，没有新增运行依赖。

![默认无手的星芽与独立配色](starpet-minimal.png)

## 验证与限制

- 本轮 Node 测试 21 项、前端生产构建、desktop 检查及正式 Apple Silicon app / DMG 打包通过；Rust 无 desktop 测试 30 项通过、1 项既有本机 CLI 测试忽略。运行时生成的 ACL 检查通过，星伴窗口仍只能访问星伴专用命令。
- 浏览器已查看完整 32 种表情列表，切换到「一起做事」显示 12 项；开启及暂停巡游成功。既有 DOM 检查确认身体变换与 SVG 路径 `d` 确实变化，默认展示为 1 个动画主预览与 32 个静态缩略图。本轮测试补充贝塞尔空间曲线端点、非直线中点、连续衔接与数值稳定性。
- 本轮隔离原生应用 `soft-verify` 使用 0 条历史、暂停记录，已核对紫色星芽换到 `curious` 后仍保色、触碰及打开主界面；顶栏主题即时切到青苔后正常退出重启，`forest` 主题、`curious` 表情、紫色与 `enabled: true` 均恢复。浮窗开启时，在设置选蓝色也能实时更新浮窗。
- 前版星伴的收起操作已补测，收起后主程序继续保留；拖动工具未报错，但没有绝对位置读回，不记录为精确移动位置验收。托盘和原生关闭事件尚未完成验收。此前供用户手动操作的旧版 pet QA 窗口未动；本轮 soft QA 正常退出，暂留产物。
- 原生窗口截图被工具扁平化为 RGB 白底，不能据此确认实际桌面透显效果。位置目前不持久化；不承诺逐像素鼠标穿透或跨全屏显示，多显示器、Windows 和长期资源占用仍未验证。
- 本轮 `artifacts/card-studio/Starclip.app` 与同目录 DMG 已安全替换，正式启动核实外观即时保存文案和星伴 5 色入口。用户已保存的星伴开关、配色、表情、采集与贴纸设置保留；升级前后 `clips`、`groups`、`preferences`、`handwriting-app.json` 及 `renderers/xingjian` 摘要一致。旧正式 app / DMG 已移入废纸篓，可恢复。

![上一版圆润星芽的原生窗口截图，手部已在后续简化；白底不作为透明视觉证据](starpet-rounded-native.png)

当前正式 app 为 9,064,495 字节（8.65 MiB），DMG 为 3,964,366 字节（3.78 MiB）；这次本体拖动更新后 app 体积不变。未引入新的运行依赖，以上体积不包含独立手写组件、用户历史和 WebView 缓存。其他证据见 [验证记录](verification.md)；完整表情展示见 [表情墙截图](starpet-expressions.png)。

## 参考与许可边界

| 参考 | 已核实内容与本项目取舍 |
| --- | --- |
| [xAI 官方仓库](https://github.com/orgs/xai-org/repositories) / [Grok Bot 文档](https://docs.x.ai/grok-bot/overview) | 未发现官方公开完整表情源码、角色资产及再使用许可。[grok-prompts](https://github.com/xai-org/grok-prompts) 是提示词，不是表情系统。不能把第三方复刻或其 32 种状态说成官方完整开源表情。 |
| [sam70361/aora-bot](https://github.com/sam70361/aora-bot) | [根 LICENSE](https://github.com/sam70361/aora-bot/blob/main/LICENSE) 默认限非商业；球形角色视觉明确不提供商业授权，引擎和表情数据可另获授权。[商业条款](https://github.com/sam70361/aora-bot/blob/main/LICENSE-COMMERCIAL.md) 是模板，仍需单独书面确认。云宝 / 星星亮亮受独立 [mood-mates/LICENSE](https://github.com/sam70361/aora-bot/blob/main/mood-mates/LICENSE) 约束，也不是免费商用素材。本项目不移植。 |
| [tycoding/grok-ball](https://github.com/tycoding/grok-ball) | [LICENSE](https://github.com/tycoding/grok-ball/blob/main/LICENSE) 为署名 tycoding 的 MIT，允许在保留许可与版权声明等条件下使用和商用。[README](https://github.com/tycoding/grok-ball/blob/main/README.md) 自称原创实现，并明确与 xAI / Grok 无关；这是作者对其项目的许可，不是 xAI 官方表情资产授权。本次未审计源码或参数来源，星芽仍独立实现。 |
| [LorisYounger/VPet](https://github.com/LorisYounger/VPet) | 仓库 [LICENSE](https://github.com/LorisYounger/VPet/blob/main/LICENSE) 为 Apache-2.0。仅作为桌宠产品形态参考，不引入其引擎、角色或素材；本次没有审计各项素材的单独权利声明。 |

[emotion-balls.vercel.app](https://emotion-balls.vercel.app/) 在上述 aora-bot 及 [xiaoxiaohaozai/emotion-ball](https://github.com/xiaoxiaohaozai/emotion-ball) 仓库中均被列为演示站；后者明确是前者的 fork，其 [LICENSE](https://github.com/xiaoxiaohaozai/emotion-ball/blob/main/LICENSE) 同样禁止未经另行授权的商业用途。后续已通过浏览器实际查看球球、云宝、亮亮的 32 状态展示及页脚许可说明，仍不据此推断当前部署对应哪个提交。

源码可见不等于 OSI 开源。带非商业限制的社区许可不符合 [OSI 开源定义第 6 条](https://opensource.org/osd)；GitHub 的 `NOASSERTION` 等识别结果不能替代许可证正文。仓库中的许可也不能单独证明作者拥有其他产品角色、商标或素材的授权。

调研读取公开 README、LICENSE、官方资料与演示网页，没有下载运行第三方桌面程序，也没有复制参考表情参数。上述许可记录不将第三方项目视为 xAI 官方授权，功能完成度以上述验证范围为准。
