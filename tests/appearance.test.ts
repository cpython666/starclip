import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';
import { applyAppearanceToRoot, appearanceStyles, contrastRatio, defaultAppearance, inspectStickerPng, maxStickerInputBytes, readableAccent, validateAppearancePatch } from '../src/appearance.ts';
import type { BaseTheme } from '../src/appearance.ts';

const backgrounds: Record<BaseTheme, string[]> = {
  dark: ['#11121b', '#14151f', '#1b1d29', '#171923', '#292c3c'],
  light: ['#f7f7fa', '#f0f0f5', '#ffffff', '#f1f1f7', '#e8e8f1'],
  white: ['#ffffff', '#f5f5f5', '#ededed'],
  forest: ['#f3f7ef', '#eaf0e3', '#fcfdf9', '#dce7d1'],
  rose: ['#fbf4f3', '#f5e9e7', '#fffafa', '#f6eae8', '#edd8d8'],
};
test('custom accent stays readable on every base-theme surface and both button states', () => {
  for (const theme of Object.keys(backgrounds) as BaseTheme[]) for (const color of ['#ffffff', '#000000', '#ff0000', '#0000ff', '#ffff00', '#d9a7c7', '#1b1d29']) {
    const accent = readableAccent(color, theme);
    const styles = appearanceStyles({ ...defaultAppearance(), accent: color }, theme);
    for (const surface of [...backgrounds[theme], styles['--gold-dim']!]) assert.ok(contrastRatio(accent, surface) >= 4.5, `${theme} ${color} on ${surface}`);
    for (const background of [styles['--primary']!, styles['--primary-hover']!]) assert.ok(contrastRatio(styles['--on-accent']!, background) >= 4.5);
  }
});

test('custom backdrops keep tested faint, muted and gold tokens readable at black and white overlay extremes', () => {
  const originals: Record<BaseTheme, Record<string, string>> = {
    dark: { faint: '#9095ac', muted: '#adb1c5', gold: '#d8bf8b' },
    light: { faint: '#63697b', muted: '#5b6175', gold: '#805c2b' },
    white: { faint: '#686868', muted: '#555555', gold: '#404040' },
    forest: { faint: '#5c6c56', muted: '#4d6552', gold: '#376a3f' },
    rose: { faint: '#82616b', muted: '#775765', gold: '#91465f' },
  };
  for (const theme of Object.keys(backgrounds) as BaseTheme[]) {
    for (const extreme of [0, 255]) {
      const shaded = '#' + [1, 3, 5].map(offset => Math.round(parseInt(backgrounds[theme][0]!.slice(offset, offset + 2), 16) * 0.94 + extreme * 0.06).toString(16).padStart(2, '0')).join('');
      for (const accent of [null, '#000000', '#ffffff', '#d9a7c7', '#4d79bb']) {
        const styles = appearanceStyles({ ...defaultAppearance(), accent, background: 'solid', colorStart: extreme ? '#ffffff' : '#000000' }, theme);
        for (const token of ['faint', 'muted', 'gold']) {
          const color = styles[`--${token}`] || originals[theme][token]!;
          assert.ok(contrastRatio(color, shaded) >= 4.5, `${theme} ${token} on ${shaded}`);
        }
      }
    }
  }
  const rose = appearanceStyles({ ...defaultAppearance(), background: 'gradient', colorStart: '#000000', colorEnd: '#ffffff' }, 'rose');
  assert.notEqual(rose['--faint'], originals.rose.faint, 'the previously failing rose label is adjusted');
  assert.equal(appearanceStyles(defaultAppearance(), 'rose')['--faint'], undefined, 'base theme needs no custom faint override');
});

test('appearance reset removes every old custom token without replacing the base palette', () => {
  const values = new Map<string, string>([['--panel', 'original-panel']]);
  const root = { style: { setProperty: (key: string, value: string) => values.set(key, value), removeProperty: (key: string) => values.delete(key) } };
  applyAppearanceToRoot({ ...defaultAppearance(), accent: '#ffffff', background: 'gradient', angle: 270 }, 'rose', root as unknown as HTMLElement);
  assert.equal(values.get('--appearance-background'), 'linear-gradient(270deg, #d9a7c7, #fffcdc)');
  assert.equal(values.get('--panel'), 'original-panel');
  assert.ok(values.has('--faint'), 'custom background installs only the needed text correction');
  applyAppearanceToRoot(defaultAppearance(), 'white', root as unknown as HTMLElement);
  assert.deepEqual([...values.keys()].sort(), ['--on-danger', '--panel']);
  const unsafe = appearanceStyles({ ...defaultAppearance(), background: 'gradient', colorStart: 'url(https://invalid.example)', angle: Infinity }, 'dark');
  assert.equal(unsafe['--appearance-background'], 'linear-gradient(90deg, #d9a7c7, #fffcdc)');
  assert.deepEqual(validateAppearancePatch({ accent: null, angle: 0, stickerEnabled: false }), { accent: null, angle: 0, stickerEnabled: false });
  for (const invalid of [{ angle: 360 }, { angle: 1.5 }, { colorStart: '#abc' }, { accent: 'red' }, { background: 'url(x)' }, { stickerDataUrl: 'data:image/svg+xml,...' }, { stickerEnabled: null }]) assert.throws(() => validateAppearancePatch(invalid as any));
});

const png = new Uint8Array(fs.readFileSync(new URL('../public/share/overview.png', import.meta.url)));
test('PNG preflight bounds encoded and decoded size and rejects animation, disguised files and trailing data', () => {
  assert.deepEqual(inspectStickerPng(png), { width: 1080, height: 1440 });
  assert.throws(() => inspectStickerPng(new Uint8Array(maxStickerInputBytes + 1)), /5 MiB/);
  assert.throws(() => inspectStickerPng(new TextEncoder().encode('<svg><script>example</script></svg>')), /PNG/);
  const oversized = png.slice(); new DataView(oversized.buffer).setUint32(16, 4096);
  assert.throws(() => inspectStickerPng(oversized), /2048/);
  const tooManyPixels = png.slice(); const view = new DataView(tooManyPixels.buffer); view.setUint32(16, 2048); view.setUint32(20, 2048);
  assert.throws(() => inspectStickerPng(tooManyPixels), /400 万/);
  assert.throws(() => inspectStickerPng(png.slice(0, -1)), /不完整/);
  const animated = png.slice(); animated.set([97, 99, 84, 76], 37);
  assert.throws(() => inspectStickerPng(animated), /动画/);
  assert.throws(() => inspectStickerPng(new Uint8Array([...png, 0])), /额外数据/);
});

let sequence = 0;
async function loadApi(desktop: boolean) {
  Object.assign(globalThis, { window: desktop ? { __TAURI_INTERNALS__: {} } : {} });
  const helper = ts.transpileModule(fs.readFileSync(new URL('../src/appearance.ts', import.meta.url), 'utf8'), { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
  const helperUrl = 'data:text/javascript;base64,' + Buffer.from(helper).toString('base64');
  const source = fs.readFileSync(new URL('../src/appearance-api.ts', import.meta.url), 'utf8').replaceAll("'./appearance'", `'${helperUrl}'`).replaceAll("import('@tauri-apps/api/core')", 'Promise.resolve(globalThis.appearanceTestCore)');
  const code = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText;
  return import('data:text/javascript;base64,' + Buffer.from(code + `\n// ${sequence++}`).toString('base64'));
}
function file(bytes = png, type = 'image/png') { return new File([bytes.slice().buffer], '原创样例.png', { type }); }
test('desktop appearance IPC sends only validated patches and PNG bytes, and propagates native failures', async () => {
  const calls: unknown[] = []; let rejectSave = false;
  Object.assign(globalThis, { appearanceTestCore: { invoke: async (command: string, args: unknown) => { calls.push([command, args]); if (rejectSave) throw new Error('test write failure'); return defaultAppearance(); } } });
  const api = await loadApi(true);
  await api.getAppearance(); await api.saveAppearance({ accent: '#AABBCC', angle: 315 });
  assert.deepEqual(calls, [['get_appearance', undefined], ['save_appearance', { appearance: { accent: '#aabbcc', angle: 315 } }]]);
  await assert.rejects(api.saveAppearance({ stickerDataUrl: 'bad' }), /不支持/);
  await assert.rejects(api.importAppearanceSticker(file(new TextEncoder().encode('<svg/>'), 'image/svg+xml')), /PNG/);
  assert.equal(calls.length, 2);
  await api.importAppearanceSticker(file());
  const imported = calls[2] as [string, { pngBase64: string; name: string }];
  assert.equal(imported[0], 'import_appearance_sticker'); assert.equal(imported[1].name, '原创样例.png');
  assert.deepEqual(new Uint8Array(Buffer.from(imported[1].pngBase64, 'base64')), png);
  rejectSave = true; await assert.rejects(api.saveAppearance({ angle: 90 }), /test write failure/);
});

test('browser preview stays memory-only and a failed replacement preserves the previous sticker', async () => {
  let decodeFails = false, outputTooBig = false, revoked = 0; const sizes: number[][] = [];
  Object.assign(globalThis, {
    Image: class { src = ''; naturalWidth = 1080; naturalHeight = 1440; async decode() { if (decodeFails) throw new Error('decode failed'); } },
    document: { createElement(tag: string) { assert.equal(tag, 'canvas'); return { width: 0, height: 0, getContext: () => ({ drawImage(...args: any[]) { sizes.push(args.slice(1)); } }), toBlob(callback: (blob: Blob) => void) { callback(new Blob([outputTooBig ? new Uint8Array(512 * 1024 + 1) : png], { type: 'image/png' })); } }; } },
  });
  const originalRevoke = URL.revokeObjectURL; URL.revokeObjectURL = () => { revoked++; };
  try {
    const api = await loadApi(false);
    await assert.rejects(api.saveAppearance({ stickerEnabled: true }), /先导入/);
    const first = await api.importAppearanceSticker(file());
    assert.equal(first.stickerEnabled, true); assert.deepEqual(sizes[0], [0, 0, 384, 512]);
    decodeFails = true; await assert.rejects(api.importAppearanceSticker(file()), /decode failed/);
    assert.deepEqual(await api.getAppearance(), first);
    decodeFails = false; outputTooBig = true;
    await assert.rejects(api.importAppearanceSticker(file()), /512 KiB/); assert.deepEqual(await api.getAppearance(), first);
    assert.equal(revoked, 3);
    await api.saveAppearance({ accent: '#123456', stickerEnabled: false });
    const removed = await api.removeAppearanceSticker(); assert.equal(removed.stickerDataUrl, null); assert.equal(removed.accent, '#123456');
    assert.deepEqual(await (await loadApi(false)).getAppearance(), defaultAppearance(), 'a new page/module has no persisted demo data');
  } finally { URL.revokeObjectURL = originalRevoke; }
});
