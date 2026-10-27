import { defaultAppearance, inspectStickerPng, maxStickerInputBytes, maxStickerStoredBytes, validateAppearancePatch } from './appearance';
import type { Appearance, AppearancePatch } from './appearance';

export const appearanceDesktopAvailable = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
let demo = defaultAppearance();
export async function getAppearance(): Promise<Appearance> {
  if (appearanceDesktopAvailable) return (await import('@tauri-apps/api/core')).invoke('get_appearance');
  return { ...demo };
}
export async function saveAppearance(appearance: AppearancePatch): Promise<Appearance> {
  const patch = validateAppearancePatch(appearance);
  if (appearanceDesktopAvailable) return (await import('@tauri-apps/api/core')).invoke('save_appearance', { appearance: patch });
  if (patch.stickerEnabled && !demo.stickerDataUrl) throw new Error('请先导入一张 PNG 贴纸。');
  demo = { ...demo, ...patch };
  return { ...demo };
}
function toBase64(bytes: Uint8Array): string {
  let binary = '';
  for (let offset = 0; offset < bytes.length; offset += 16384) binary += String.fromCharCode(...bytes.subarray(offset, offset + 16384));
  return btoa(binary);
}
async function prepareDemoSticker(bytes: Uint8Array, width: number, height: number): Promise<string> {
  const url = URL.createObjectURL(new Blob([bytes.slice().buffer], { type: 'image/png' }));
  const image = new Image();
  try {
    image.src = url;
    await image.decode();
    if (image.naturalWidth !== width || image.naturalHeight !== height) throw new Error('贴纸尺寸与图片内容不一致。');
    const scale = Math.min(1, 512 / Math.max(width, height));
    const canvas = document.createElement('canvas');
    canvas.width = Math.max(1, Math.round(width * scale)); canvas.height = Math.max(1, Math.round(height * scale));
    const context = canvas.getContext('2d');
    if (!context) throw new Error('当前浏览器无法处理图片。');
    context.drawImage(image, 0, 0, canvas.width, canvas.height);
    const blob = await new Promise<Blob>((resolve, reject) => canvas.toBlob(value => value ? resolve(value) : reject(new Error('贴纸转换失败。')), 'image/png'));
    if (blob.size > maxStickerStoredBytes) throw new Error('压缩后的贴纸仍超过 512 KiB，请换一张更简洁的图片。');
    return 'data:image/png;base64,' + toBase64(new Uint8Array(await blob.arrayBuffer()));
  } finally { URL.revokeObjectURL(url); }
}
export async function importAppearanceSticker(file: File): Promise<Appearance> {
  if (!file.size || file.size > maxStickerInputBytes) throw new Error('请选择不超过 5 MiB 的静态 PNG 贴纸。');
  if (file.type && file.type !== 'image/png') throw new Error('目前仅支持静态 PNG，可保留透明背景。');
  const bytes = new Uint8Array(await file.arrayBuffer());
  const { width, height } = inspectStickerPng(bytes);
  const name = [...file.name.replace(/[\u0000-\u001f\u007f]/g, '').trim()].slice(0, 80).join('') || '自定义贴纸.png';
  if (appearanceDesktopAvailable) return (await import('@tauri-apps/api/core')).invoke('import_appearance_sticker', { pngBase64: toBase64(bytes), name });
  const stickerDataUrl = await prepareDemoSticker(bytes, width, height);
  demo = { ...demo, stickerEnabled: true, stickerName: name, stickerDataUrl };
  return { ...demo };
}
export async function removeAppearanceSticker(): Promise<Appearance> {
  if (appearanceDesktopAvailable) return (await import('@tauri-apps/api/core')).invoke('remove_appearance_sticker');
  demo = { ...demo, stickerEnabled: false, stickerName: null, stickerDataUrl: null };
  return { ...demo };
}
