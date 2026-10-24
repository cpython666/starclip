# 星芽圆润形象设计

先用内置 imagegen 生成概念图，再人工按图把轮廓、五个圆润星角、小圆手、眼睛和高光还原为可变色 SVG。应用运行时不加载这张位图；它仅保存在设计资料中。

- 概念图：`starpet-concept.png`
- 实际 SVG 实界面：`starpet-rounded.png`、`starpet-rounded-native.png`
- 角色源文件：`../src/components/StarPet.vue`
- 三次贝塞尔轮廓、空间跳跃与弹簧：`../src/starpet-motion.ts`
- 动作参考：本机 game-feel 技能的预备动作、挤压拉伸、缓入缓出、延迟跟随与回稳；本机未找到明确名为迪士尼十二动画原则的技能。

## 最终生图提示词

使用内置 imagegen，非 CLI。原始输出保持不变。

```text
Use case: stylized-concept. Asset type: original mascot model sheet to guide a lightweight SVG desktop pet. Primary request: design an extremely cute, rounded, lively little five-pointed star named Xingya for a cozy notes app. Front-facing inflated marshmallow / soft gummy star silhouette, five short very rounded lobes, broad round belly, absolutely no sharp points. Tiny cocoa-black shiny oval eyes spaced apart on the lower-middle face, tiny curved smile, soft peach blush, two small rounded mitten arms at the sides. Warm cream to honey-yellow shading, gentle large highlight on upper left, restrained underbelly shading; vector-friendly shapes and gradients rather than realistic texture. NOT a geometric sharp star, NOT a ball with spikes, NOT a famous existing character. Charming hand-shaped slightly asymmetric contour, warm approachable and squishy. Composition: clean landscape concept board on soft ivory, one large front resting pose on the left (about half the image), then 3 smaller consistent poses on right: happy squashed bounce anticipation, joyful airborne stretched star following an arc with tiny curved motion trails, soft landing with squash. At bottom a small row of the same star in pastel peach pink, mint green, lavender, sky blue. No typography, no text, no watermark, no UI. Polished illustration, simple SVG reproducible lighting, no complex environment or accessories. Each silhouette fully separated, unclipped, plenty of whitespace.
```
