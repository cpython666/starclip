export const starPetGroups = [
  { id: 'daily', name: '日常陪伴', description: '醒来、伸个懒腰，把平常的一天过得柔软一点。' },
  { id: 'emotion', name: '小小心情', description: '开心会发光，难过也可以慢慢说。' },
  { id: 'work', name: '一起做事', description: '读一页、写几行，星芽陪你专注，也提醒你休息。' },
] as const;

export interface StarPetExpression {
  id: string;
  name: string;
  description: string;
  group: typeof starPetGroups[number]['id'];
  eyes: 'open' | 'wink' | 'wide' | 'soft' | 'closed' | 'half' | 'sleep' | 'stars' | 'hearts' | 'uneven' | 'droop' | 'squeezed' | 'focused' | 'side';
  mouth: 'smile' | 'grin' | 'tiny' | 'o' | 'yawn' | 'flat' | 'wave' | 'frown' | 'wail' | 'smirk' | 'tongue' | 'breath';
  prop: 'none' | 'wave' | 'question' | 'sound' | 'moon' | 'zzz' | 'dawn' | 'stretch' | 'spark' | 'sparkles' | 'hearts' | 'flower' | 'surprise' | 'puzzle' | 'cloud' | 'tears' | 'anger' | 'sweat' | 'crown' | 'sigh' | 'thought' | 'magnifier' | 'book' | 'pencil' | 'palette' | 'target' | 'hourglass' | 'orbit' | 'check' | 'alert' | 'confetti' | 'mug';
  motion: 'float' | 'wave' | 'tilt' | 'breathe' | 'stretch' | 'bounce' | 'hop' | 'shiver' | 'nod' | 'scan' | 'work' | 'still' | 'party';
  tilt: number;
  blush: number;
  accent: string;
}

// Original expression direction for 星芽. IDs are persisted; keep them stable.
export const starPetMoods = [
  { id: 'idle', name: '静静陪你', description: '轻轻浮着，把一小片夜空留在身边。', group: 'daily', eyes: 'open', mouth: 'smile', prop: 'none', motion: 'float', tilt: -2, blush: .38, accent: '#8eb6c5' },
  { id: 'greet', name: '挥手问好', description: '举起小星角，向刚来的你打个招呼。', group: 'daily', eyes: 'wink', mouth: 'grin', prop: 'wave', motion: 'wave', tilt: -5, blush: .56, accent: '#93c8b9' },
  { id: 'curious', name: '有点好奇', description: '歪着脑袋，想听听接下来的故事。', group: 'daily', eyes: 'wide', mouth: 'tiny', prop: 'question', motion: 'tilt', tilt: -10, blush: .3, accent: '#a99bcf' },
  { id: 'listening', name: '认真听你', description: '靠近一点，连很轻的话也好好接住。', group: 'daily', eyes: 'soft', mouth: 'flat', prop: 'sound', motion: 'nod', tilt: 7, blush: .3, accent: '#8fb9ce' },
  { id: 'sleepy', name: '开始犯困', description: '眼皮慢慢落下，月亮也来陪着打哈欠。', group: 'daily', eyes: 'half', mouth: 'yawn', prop: 'moon', motion: 'tilt', tilt: -5, blush: .36, accent: '#a6a4d0' },
  { id: 'sleeping', name: '安稳睡着', description: '呼吸像小波浪，梦里也有柔软的星光。', group: 'daily', eyes: 'sleep', mouth: 'tiny', prop: 'zzz', motion: 'breathe', tilt: 10, blush: .25, accent: '#a3b4cf' },
  { id: 'waking', name: '刚刚醒来', description: '一只眼先醒了，另一只还想再赖一会儿。', group: 'daily', eyes: 'wink', mouth: 'yawn', prop: 'dawn', motion: 'nod', tilt: -4, blush: .42, accent: '#e7b777' },
  { id: 'stretch', name: '伸个懒腰', description: '把五个小星角舒展开，迎接新的一页。', group: 'daily', eyes: 'squeezed', mouth: 'grin', prop: 'stretch', motion: 'stretch', tilt: 0, blush: .42, accent: '#a1c8b4' },
  { id: 'happy', name: '心里晴朗', description: '眼睛笑成小月牙，脚下也轻快起来。', group: 'emotion', eyes: 'closed', mouth: 'grin', prop: 'spark', motion: 'bounce', tilt: -3, blush: .6, accent: '#e7b26e' },
  { id: 'delighted', name: '闪闪开心', description: '眼里亮起小星星，藏不住满满的欢喜。', group: 'emotion', eyes: 'stars', mouth: 'grin', prop: 'sparkles', motion: 'hop', tilt: 3, blush: .65, accent: '#dcb675' },
  { id: 'love', name: '喜欢你呀', description: '把心里的喜欢，变成一颗颗小爱心。', group: 'emotion', eyes: 'hearts', mouth: 'smile', prop: 'hearts', motion: 'float', tilt: -4, blush: .75, accent: '#dd8fa4' },
  { id: 'shy', name: '悄悄害羞', description: '脸颊热了一点，借一朵小花挡挡视线。', group: 'emotion', eyes: 'half', mouth: 'tiny', prop: 'flower', motion: 'tilt', tilt: 10, blush: .9, accent: '#dca0b3' },
  { id: 'surprised', name: '哇的一下', description: '眼睛睁圆，刚刚是不是发现了什么？', group: 'emotion', eyes: 'wide', mouth: 'o', prop: 'surprise', motion: 'hop', tilt: 0, blush: .2, accent: '#ddb37c' },
  { id: 'puzzled', name: '有点迷糊', description: '一个问号，又一个问号，让我想一想。', group: 'emotion', eyes: 'uneven', mouth: 'wave', prop: 'puzzle', motion: 'tilt', tilt: 9, blush: .24, accent: '#b49fce' },
  { id: 'sad', name: '低落一会', description: '小云停在头顶，今天也可以不那么坚强。', group: 'emotion', eyes: 'droop', mouth: 'frown', prop: 'cloud', motion: 'breathe', tilt: -5, blush: .2, accent: '#92abc5' },
  { id: 'crying', name: '眼泪汪汪', description: '让眼泪流一会儿，难过会慢慢变轻。', group: 'emotion', eyes: 'squeezed', mouth: 'wail', prop: 'tears', motion: 'shiver', tilt: 0, blush: .58, accent: '#84b7d3' },
  { id: 'angry', name: '气鼓鼓的', description: '小星角绷起来了，但还是软乎乎的一颗。', group: 'emotion', eyes: 'focused', mouth: 'frown', prop: 'anger', motion: 'shiver', tilt: -3, blush: .85, accent: '#d78677' },
  { id: 'nervous', name: '有些紧张', description: '攥紧小手，深呼吸一下再出发。', group: 'emotion', eyes: 'wide', mouth: 'wave', prop: 'sweat', motion: 'shiver', tilt: 3, blush: .38, accent: '#8eb7ce' },
  { id: 'proud', name: '小小得意', description: '戴上自己的小皇冠：这次做得不错呢。', group: 'emotion', eyes: 'soft', mouth: 'smirk', prop: 'crown', motion: 'nod', tilt: -7, blush: .48, accent: '#d2ab66' },
  { id: 'relieved', name: '松一口气', description: '心里的结解开了，肩膀也轻了一点。', group: 'emotion', eyes: 'sleep', mouth: 'breath', prop: 'sigh', motion: 'breathe', tilt: 4, blush: .3, accent: '#a3c2bb' },
  { id: 'thinking', name: '慢慢思考', description: '让想法在小云朵里转一会儿。', group: 'work', eyes: 'half', mouth: 'wave', prop: 'thought', motion: 'tilt', tilt: -8, blush: .26, accent: '#afa4cb' },
  { id: 'searching', name: '找找线索', description: '举起放大镜，每个小细节都值得看一眼。', group: 'work', eyes: 'side', mouth: 'o', prop: 'magnifier', motion: 'scan', tilt: 5, blush: .25, accent: '#8cb9bd' },
  { id: 'reading', name: '读一小页', description: '翻开小书，跟着文字去远一点的地方。', group: 'work', eyes: 'soft', mouth: 'tiny', prop: 'book', motion: 'breathe', tilt: -2, blush: .3, accent: '#8faebd' },
  { id: 'writing', name: '写下几行', description: '握好小铅笔，把闪过的念头留下来。', group: 'work', eyes: 'focused', mouth: 'tiny', prop: 'pencil', motion: 'work', tilt: -5, blush: .35, accent: '#92b9ae' },
  { id: 'drawing', name: '涂点颜色', description: '一点蓝，一点粉，画出自己的小宇宙。', group: 'work', eyes: 'wink', mouth: 'tongue', prop: 'palette', motion: 'work', tilt: 5, blush: .55, accent: '#ca9caf' },
  { id: 'focused', name: '专心此刻', description: '把注意力收回来，先做好眼前这一件。', group: 'work', eyes: 'focused', mouth: 'flat', prop: 'target', motion: 'still', tilt: 0, blush: .18, accent: '#83b9b6' },
  { id: 'waiting', name: '耐心等等', description: '看着沙漏，留一点时间给正在发生的事。', group: 'work', eyes: 'side', mouth: 'flat', prop: 'hourglass', motion: 'nod', tilt: 4, blush: .22, accent: '#b0a2cc' },
  { id: 'loading', name: '正在准备', description: '小行星绕着转，好东西正在慢慢到来。', group: 'work', eyes: 'open', mouth: 'tiny', prop: 'orbit', motion: 'float', tilt: -2, blush: .3, accent: '#9cb5ce' },
  { id: 'success', name: '做好啦', description: '亮出一个小勾，认真完成的事值得开心。', group: 'work', eyes: 'closed', mouth: 'grin', prop: 'check', motion: 'bounce', tilt: -4, blush: .55, accent: '#84b89f' },
  { id: 'error', name: '再试一次', description: '遇到小岔路了，停一停，再一起找办法。', group: 'work', eyes: 'uneven', mouth: 'wave', prop: 'alert', motion: 'tilt', tilt: 5, blush: .42, accent: '#d39a83' },
  { id: 'celebrate', name: '撒点星光', description: '让彩纸和星光飞起来，为这一刻庆祝。', group: 'work', eyes: 'stars', mouth: 'grin', prop: 'confetti', motion: 'party', tilt: 0, blush: .72, accent: '#d6a6c2' },
  { id: 'resting', name: '歇一小会', description: '捧着温热的小杯子，让自己缓一缓。', group: 'work', eyes: 'sleep', mouth: 'smile', prop: 'mug', motion: 'breathe', tilt: 5, blush: .36, accent: '#a1bfa9' },
] as const satisfies readonly StarPetExpression[];

export type StarPetMoodId = typeof starPetMoods[number]['id'];
export function getStarPetMood(id: string) { return starPetMoods.find(mood => mood.id === id) ?? starPetMoods[0]; }
