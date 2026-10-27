import catalog from './product-share-catalog.json' with { type: 'json' };

export const productShareDesktopAvailable = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
export const shareTexts = catalog.texts;
export const shareImages = catalog.images;
export type ShareImage = typeof shareImages[number];
export type ShareSaveResult = { kind: 'saved'; path: string } | { kind: 'cancelled' } | { kind: 'downloaded'; filename: string };
const maxImageBytes = 3 * 1024 * 1024;

export function shareImageSource(image: ShareImage): string { return `/${image.path}`; }
export function shareImageSize(bytes: number): string { return `${Math.ceil(bytes / 1024)} KB`; }
function requiredImage(id: string): ShareImage {
  const image = shareImages.find(item => item.id === id);
  if (!image) throw new Error('这张产品图片暂不可用。');
  return image;
}
async function imageBlob(image: ShareImage): Promise<Blob> {
  const response = await fetch(shareImageSource(image));
  if (!response.ok || Number(response.headers.get('content-length')) > maxImageBytes) throw new Error('产品图片读取失败，请重试。');
  const blob = await response.blob();
  if (blob.type !== 'image/png' || !blob.size || blob.size > maxImageBytes) throw new Error('产品图片文件无效或过大。');
  return blob;
}

export async function copyProductShareText(id: string): Promise<void> {
  const text = shareTexts.find(item => item.id === id);
  if (!text) throw new Error('这段分享文案暂不可用。');
  if (productShareDesktopAvailable) return (await import('@tauri-apps/api/core')).invoke('copy_product_share_text', { id });
  if (!navigator.clipboard?.writeText) throw new Error('当前浏览器不支持一键复制，请选中文案手动复制。');
  try { await navigator.clipboard.writeText(text.content); }
  catch { throw new Error('文案未能复制，请允许剪贴板权限，或选中文案手动复制。'); }
}

export async function copyProductShareImage(id: string): Promise<void> {
  const image = requiredImage(id);
  if (productShareDesktopAvailable) return (await import('@tauri-apps/api/core')).invoke('copy_product_share_image', { id });
  if (!navigator.clipboard?.write || typeof ClipboardItem === 'undefined') throw new Error('当前浏览器不支持复制图片，可以先保存图片再分享。');
  // Start clipboard.write in the click's user gesture; Safari accepts the promised PNG.
  const png = imageBlob(image);
  void png.catch(() => {});
  try { await navigator.clipboard.write([new ClipboardItem({ 'image/png': png })]); }
  catch { throw new Error('图片未能复制，请允许剪贴板权限，或使用「保存图片」。'); }
}

export async function saveProductShareImage(id: string): Promise<ShareSaveResult> {
  const image = requiredImage(id);
  if (productShareDesktopAvailable) {
    const path = await (await import('@tauri-apps/api/core')).invoke<string | null>('save_product_share_image', { id });
    return path ? { kind: 'saved', path } : { kind: 'cancelled' };
  }
  const blob = await imageBlob(image);
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url; link.download = image.filename;
  try { link.click(); } finally { setTimeout(() => URL.revokeObjectURL(url), 1000); }
  return { kind: 'downloaded', filename: image.filename };
}
