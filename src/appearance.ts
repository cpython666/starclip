export interface Appearance {
  accent: string | null;
  background: 'theme' | 'solid' | 'gradient';
  colorStart: string;
  colorEnd: string;
  angle: number;
  stickerEnabled: boolean;
  stickerName: string | null;
  stickerDataUrl: string | null;
}
export type AppearancePatch = Partial<Pick<Appearance, 'accent' | 'background' | 'colorStart' | 'colorEnd' | 'angle' | 'stickerEnabled'>>;
export type BaseTheme = 'dark' | 'light' | 'white' | 'forest' | 'rose';
export function defaultAppearance(): Appearance {
  return { accent: null, background: 'theme', colorStart: '#d9a7c7', colorEnd: '#fffcdc', angle: 90, stickerEnabled: false, stickerName: null, stickerDataUrl: null };
}
export const appearancePresets = [
  { name: '柔粉奶油', colorStart: '#d9a7c7', colorEnd: '#fffcdc', angle: 90 },
  { name: '落日莓果', colorStart: '#ff512f', colorEnd: '#dd2476', angle: 135 },
  { name: '深海微光', colorStart: '#184e68', colorEnd: '#57ca85', angle: 90 },
  { name: '星云晚霞', colorStart: '#5f42a4', colorEnd: '#df8baa', angle: 135 },
  { name: '晴空薄荷', colorStart: '#7db9e8', colorEnd: '#a8e6cf', angle: 45 },
] as const;
export const gradientDirections = [
  { value: 0, label: '↑ 向上' }, { value: 45, label: '↗ 右上' },
  { value: 90, label: '→ 向右' }, { value: 135, label: '↘ 右下' },
  { value: 180, label: '↓ 向下' }, { value: 225, label: '↙ 左下' },
  { value: 270, label: '← 向左' }, { value: 315, label: '↖ 左上' },
] as const;

function hex(value: unknown): value is string { return typeof value === 'string' && /^#[0-9a-f]{6}$/i.test(value); }
export function validateAppearancePatch(value: AppearancePatch): AppearancePatch {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('外观设置格式不正确。');
  const result: AppearancePatch = {};
  for (const [key, field] of Object.entries(value)) {
    if (key === 'accent') {
      if (field !== null && !hex(field)) throw new Error('主题色需为六位 HEX，例如 #9977FF。');
      result.accent = field === null ? null : field.toLowerCase();
    } else if (key === 'colorStart' || key === 'colorEnd') {
      if (!hex(field)) throw new Error('背景颜色需为六位 HEX。');
      result[key] = field.toLowerCase();
    } else if (key === 'background') {
      if (field !== 'theme' && field !== 'solid' && field !== 'gradient') throw new Error('背景模式不受支持。');
      result.background = field;
    } else if (key === 'angle') {
      if (!Number.isInteger(field) || (field as number) < 0 || (field as number) > 359) throw new Error('渐变方向需为 0–359 的整数角度。');
      result.angle = field as number;
    } else if (key === 'stickerEnabled') {
      if (typeof field !== 'boolean') throw new Error('贴纸开关格式不正确。');
      result.stickerEnabled = field;
    } else throw new Error('不支持这项外观设置。');
  }
  return result;
}

const surfaces: Record<BaseTheme, string[]> = {
  dark: ['#11121b', '#14151f', '#1b1d29', '#171923', '#292c3c'],
  light: ['#f7f7fa', '#f0f0f5', '#ffffff', '#f1f1f7', '#e8e8f1'],
  white: ['#ffffff', '#f5f5f5', '#ededed'],
  forest: ['#f3f7ef', '#eaf0e3', '#fcfdf9', '#dce7d1'],
  rose: ['#fbf4f3', '#f5e9e7', '#fffafa', '#f6eae8', '#edd8d8'],
};
const textTokens: Record<BaseTheme, { faint: string; muted: string; gold: string }> = {
  dark: { faint: '#9095ac', muted: '#adb1c5', gold: '#d8bf8b' },
  light: { faint: '#63697b', muted: '#5b6175', gold: '#805c2b' },
  white: { faint: '#686868', muted: '#555555', gold: '#404040' },
  forest: { faint: '#5c6c56', muted: '#4d6552', gold: '#376a3f' },
  rose: { faint: '#82616b', muted: '#775765', gold: '#91465f' },
};
function rgb(color: string) { return [1, 3, 5].map(offset => parseInt(color.slice(offset, offset + 2), 16)); }
function mix(a: string, b: string, amount: number) {
  const first = rgb(a), second = rgb(b);
  return '#' + first.map((channel, index) => Math.round(channel * (1 - amount) + second[index]! * amount).toString(16).padStart(2, '0')).join('');
}
function luminance(color: string) {
  const channels = rgb(color).map(channel => { const value = channel / 255; return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4; });
  return channels[0]! * 0.2126 + channels[1]! * 0.7152 + channels[2]! * 0.0722;
}
export function contrastRatio(a: string, b: string) {
  const first = luminance(a), second = luminance(b);
  return (Math.max(first, second) + 0.05) / (Math.min(first, second) + 0.05);
}
function readableOn(color: string, theme: BaseTheme, backgrounds: string[], accentDim = false): string {
  const base = hex(color) ? color.toLowerCase() : '#9977ff';
  const target = theme === 'dark' ? '#ffffff' : '#000000';
  for (let step = 0; step <= 100; step++) {
    const candidate = mix(base, target, step / 100);
    const checked = accentDim ? [...backgrounds, mix(surfaces[theme][0]!, candidate, 0.12)] : backgrounds;
    if (checked.every(background => contrastRatio(candidate, background) >= 4.5)) return candidate;
  }
  return target;
}
export function readableAccent(color: string, theme: BaseTheme, extraBackgrounds: string[] = []): string {
  return readableOn(color, theme, [...surfaces[theme], ...extraBackgrounds], true);
}

/** All generated values come from validated colors/numbers, never arbitrary CSS. */
export function appearanceStyles(value: Appearance, theme: BaseTheme): Record<string, string> {
  const danger = { dark: '#ed9da5', light: '#b23f53', white: '#b63f46', forest: '#a64d49', rose: '#b4434d' }[theme];
  const style: Record<string, string> = { '--on-danger': contrastRatio(danger, '#000000') >= contrastRatio(danger, '#ffffff') ? '#000000' : '#ffffff' };
  const start = hex(value.colorStart) ? value.colorStart.toLowerCase() : '#d9a7c7';
  const end = hex(value.colorEnd) ? value.colorEnd.toLowerCase() : '#fffcdc';
  const angle = Number.isInteger(value.angle) && value.angle >= 0 && value.angle <= 359 ? value.angle : 90;
  const extraBackgrounds: string[] = [];
  if (value.background === 'solid' || value.background === 'gradient') {
    style['--appearance-background'] = value.background === 'solid' ? start : `linear-gradient(${angle}deg, ${start}, ${end})`;
    style['--appearance-surface'] = `rgba(${rgb(surfaces[theme][0]!).join(', ')}, 0.94)`;
    // Bound the 6% of any chosen backdrop visible through the reading surface.
    extraBackgrounds.push(mix(surfaces[theme][0]!, '#000000', 0.06), mix(surfaces[theme][0]!, '#ffffff', 0.06));
    for (const token of ['faint', 'muted'] as const) {
      const original = textTokens[theme][token];
      const adjusted = readableOn(original, theme, [...surfaces[theme], ...extraBackgrounds]);
      if (adjusted !== original) style[`--${token}`] = adjusted;
    }
  }
  if (hex(value.accent)) {
    const accent = readableAccent(value.accent, theme, extraBackgrounds);
    const onAccent = contrastRatio(accent, '#000000') >= contrastRatio(accent, '#ffffff') ? '#000000' : '#ffffff';
    Object.assign(style, {
      '--gold': accent, '--gold-dim': mix(surfaces[theme][0]!, accent, 0.12),
      '--primary': accent, '--primary-hover': mix(accent, onAccent === '#000000' ? '#ffffff' : '#000000', 0.12),
      '--on-accent': onAccent, '--purple': accent,
    });
  } else if (extraBackgrounds.length) {
    const adjusted = readableAccent(textTokens[theme].gold, theme, extraBackgrounds);
    if (adjusted !== textTokens[theme].gold) style['--gold'] = adjusted;
  }
  return style;
}
const appearanceProperties = ['--appearance-background', '--appearance-surface', '--gold', '--gold-dim', '--primary', '--primary-hover', '--on-accent', '--on-danger', '--purple', '--faint', '--muted'];
export function applyAppearanceToRoot(value: Appearance, theme: BaseTheme, root: Pick<HTMLElement, 'style'> = document.documentElement) {
  const values = appearanceStyles(value, theme);
  for (const property of appearanceProperties) {
    if (values[property]) root.style.setProperty(property, values[property]); else root.style.removeProperty(property);
  }
}

export const maxStickerInputBytes = 5 * 1024 * 1024;
export const maxStickerStoredBytes = 512 * 1024;
export function inspectStickerPng(bytes: Uint8Array): { width: number; height: number } {
  const signature = [137, 80, 78, 71, 13, 10, 26, 10];
  if (bytes.length > maxStickerInputBytes) throw new Error('贴纸不能超过 5 MiB。');
  if (bytes.length < 33 || signature.some((value, index) => bytes[index] !== value)) throw new Error('请选择有效的静态 PNG 图片，暂不支持 SVG 或 GIF。');
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (view.getUint32(8) !== 13 || String.fromCharCode(...bytes.subarray(12, 16)) !== 'IHDR') throw new Error('PNG 图片头无效。');
  const width = view.getUint32(16), height = view.getUint32(20);
  if (!width || !height || width > 2048 || height > 2048 || width * height > 4_000_000) throw new Error('贴纸最多 400 万像素，边长不能超过 2048。');
  let offset = 8, ended = false;
  while (offset + 12 <= bytes.length) {
    const length = view.getUint32(offset);
    if (offset + length + 12 > bytes.length) throw new Error('PNG 图片内容不完整。');
    const type = String.fromCharCode(...bytes.subarray(offset + 4, offset + 8));
    if (type === 'acTL') throw new Error('暂不支持动画 PNG，请使用静态贴纸。');
    offset += length + 12;
    if (type === 'IEND') { ended = true; break; }
  }
  if (!ended || offset !== bytes.length) throw new Error('PNG 图片内容不完整或包含额外数据。');
  return { width, height };
}
