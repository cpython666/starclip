# 星芽圆润形象设计

先用内置 imagegen 生成概念图，再人工按图把轮廓、五个圆润星角、小圆手、眼睛和高光还原为可变色 SVG。应用运行时不加载这张位图；它仅保存在设计资料中。

- 概念图：`starpet-concept.png`
- 实际 SVG 实界面：`starpet-rounded.png`、`starpet-rounded-native.png`
- 角色源文件：`../src/components/StarPet.vue`
- 三次贝塞尔圆星轮廓、嫩芽位置、空间跳跃与弹簧：`../src/starpet-motion.ts`
- 日常动作决策、停顿与临时互动：`../src/starpet-behavior.ts`
- 动作参考：本机 game-feel 技能的预备动作、挤压拉伸、缓入缓出、延迟跟随与回稳；本机未找到明确名为迪士尼十二动画原则的技能。

## 嫩芽圆星（2026-10-31）

用户希望星芽更可爱灵动，并交给设计判断。外形改为以下方案，32 种状态、动作 ID、配色和道具位置保持不变：

- 轮廓：`starOutline` 改为按五个角参数生成的三次贝塞尔圆星；角更短更胖，凹处更浅，下方两角略短像小脚，脸落在更大的圆肚子上。顶部角保留轻微左倾的手作感，各角端点与旧版相近，拖动步态、趴边手位不需改动。
- 头顶小嫩芽：呼应「星芽」名字，两片叶子长在顶角，用欠阻尼弹簧随身体倾斜、左右与上下速度延迟摆动，并带轻微常驻摇曳；戴皇冠的「小小得意」时隐藏。
- 面部：眼睛放大并加一大一小两处高光；腮红改为径向渐变，开心时略微鼓起；描边改用稍深的同色，浅色与深色桌面上都更清楚。
- 对比图：[之前 / 之后](starpet-plump-sprout.png)，浏览器渲染的同一组姿态与配色。

## 手部简化

后续按用户偏好调整：大部分状态不显示手，只在挥手或拿东西时出现两个小圆球，不再画手掌、手指或手臂。概念图作为初版参考保留，当前 SVG 以 [无手预览](starpet-minimal.png) 与 [圆球挥手](starpet-ball-hands.png) 为准。

## 当前动作设计

保留 32 种状态、独立配色与大小设置。`starpet-behavior.ts` 在动作之间留出停顿，再根据当前心情选择观察、探头、平衡或舒展；安静和专注时动作更少，活泼心情才偶尔跳跃。跳跃沿用蓄力、弧线腾空与落地回弹，概念图中的跳跃姿态是动作参考，不是每次触摸的固定回应。

注意力从眼神开始，脸和身体稍后跟随，星角随身体变化带出柔性余动。指针进入本窗口的星芽区域时引起注视，离开后短暂保持，再渐渐收回；眨眼使用不等间隔的单眨与双眨。单次摸摸表现为眯眼倾靠，连续摸摸变为扭动，专注时点头，睡着时半醒后继续睡。动作以弹簧衔接，不重置当前姿态与速度；临时反应不轮换已选心情，也不写入数据。

图库以静态缩略图呈现，系统要求减少动态效果、关闭动画或页面隐藏时停止动画循环并保持静态。这些动作继续使用 SVG 与现有代码，不增加运行依赖。

## 最终生图提示词

使用内置 imagegen，非 CLI。原始输出保持不变。

```text
Use case: stylized-concept. Asset type: original mascot model sheet to guide a lightweight SVG desktop pet. Primary request: design an extremely cute, rounded, lively little five-pointed star named Xingya for a cozy notes app. Front-facing inflated marshmallow / soft gummy star silhouette, five short very rounded lobes, broad round belly, absolutely no sharp points. Tiny cocoa-black shiny oval eyes spaced apart on the lower-middle face, tiny curved smile, soft peach blush, two small rounded mitten arms at the sides. Warm cream to honey-yellow shading, gentle large highlight on upper left, restrained underbelly shading; vector-friendly shapes and gradients rather than realistic texture. NOT a geometric sharp star, NOT a ball with spikes, NOT a famous existing character. Charming hand-shaped slightly asymmetric contour, warm approachable and squishy. Composition: clean landscape concept board on soft ivory, one large front resting pose on the left (about half the image), then 3 smaller consistent poses on right: happy squashed bounce anticipation, joyful airborne stretched star following an arc with tiny curved motion trails, soft landing with squash. At bottom a small row of the same star in pastel peach pink, mint green, lavender, sky blue. No typography, no text, no watermark, no UI. Polished illustration, simple SVG reproducible lighting, no complex environment or accessories. Each silhouette fully separated, unclipped, plenty of whitespace.
```
