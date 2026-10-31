import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { errorText, formatNumber, hasTranslation, locale, relativeTime, resolveLocale, setLocale, t } from '../src/i18n/index.ts';
import { appearancePresets, gradientDirections } from '../src/appearance.ts';
import { sourceNames } from '../src/privacy.ts';
import { growthCatalog, growthSlots } from '../src/starpet-growth.ts';
import { starPetColors, starPetGroups, starPetMoods } from '../src/starpet.ts';
import { petStatusLabel } from '../src/starpet-signal-api.ts';
import { languages } from '../src/snippets.ts';
import { defaultCodeCardStyle } from '../src/code-card.ts';
import shareCatalog from '../src/product-share-catalog.json' with { type: 'json' };
import paperCatalog from '../public/handwriting-papers/catalog.json' with { type: 'json' };

test.afterEach(() => setLocale('zh-CN', false));

test('Chinese is the source language; English uses the catalog with interpolation and plurals', () => {
  setLocale('zh-CN', false);
  assert.equal(locale.value, 'zh-CN');
  assert.equal(t('全部记录'), '全部记录');
  assert.equal(t('来源于互联网'), '来源于互联网');
  assert.equal(t('{count} 条', { count: 3 }), '3 条');
  assert.equal(formatNumber(1000), '1,000');

  setLocale('en-US', false);
  assert.equal(t('全部记录'), 'All clips');
  assert.equal(t('来源于互联网'), 'Sourced from the internet');
  assert.equal(t('{count} 条', { count: 1 }), '1 clip');
  assert.equal(t('{count} 条', { count: 4 }), '4 clips');
  assert.equal(t('{count} 天', { count: 1 }), '1 day');
  assert.equal(t('{count} 天', { count: 7 }), '7 days');
  assert.equal(t('已导入 {count} 条记录', { count: 2 }), 'Imported 2 clips');
  assert.equal(t('用逗号分隔，例如 工作，设计'), 'Separate with commas, e.g. Work, Design');
  assert.equal(t('、'), ', ');
  assert.equal(t('，'), ', ');
  assert.equal(formatNumber(1000), '1,000');
});

test('locale resolution follows an explicit choice, otherwise Chinese in this Node fixture', () => {
  assert.equal(resolveLocale('en-US'), 'en-US');
  assert.equal(resolveLocale('zh-CN'), 'zh-CN');
  assert.equal(resolveLocale('system'), 'zh-CN');
  assert.equal(resolveLocale(undefined), 'zh-CN');
});

test('relative time is localized', () => {
  const now = Date.UTC(2026, 0, 2, 12, 0, 0);
  setLocale('zh-CN', false);
  assert.match(relativeTime(now - 2 * 60_000, now), /2\s*分钟前/);
  setLocale('en-US', false);
  assert.equal(relativeTime(now - 2 * 60_000, now), '2 minutes ago');
  assert.equal(relativeTime(now - 2 * 86_400_000, now), '2 days ago');
  assert.equal(relativeTime(now - 10_000, now), 'Just now');
});

test('native messages use exact translations, dynamic prefixes and a safe English fallback', () => {
  setLocale('zh-CN', false);
  assert.equal(errorText(new Error('快捷键格式无效，例如 CommandOrControl+Shift+V。')), '快捷键格式无效，例如 CommandOrControl+Shift+V。');
  setLocale('en-US', false);
  assert.equal(errorText(new Error('快捷键格式无效，例如 CommandOrControl+Shift+V。')), 'Invalid shortcut format. Use something like CommandOrControl+Shift+V.');
  assert.equal(errorText(new Error('无法打开主窗口：native details')), 'Could not open the main window: native details');
  assert.match(errorText(new Error('未翻译的底层错误')), /operation could not be completed/i);
  assert.equal(errorText(new Error('Plain English failure')), 'Plain English failure');
});

test('every literal t() key across the app has an English translation', () => {
  const roots = ['src'];
  const files: string[] = [];
  const walk = (directory: string) => {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      const full = path.join(directory, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (/\.(vue|ts)$/.test(entry.name) && !full.includes(`${path.sep}i18n${path.sep}`)) files.push(full);
    }
  };
  for (const root of roots) walk(root);
  const missing = new Set<string>();
  for (const file of files) {
    const source = fs.readFileSync(file, 'utf8');
    for (const match of source.matchAll(/\bt\(\s*(['"])((?:\\.|(?!\1).)*)\1/g)) {
      const key = match[2].replace(/\\'/g, "'").replace(/\\"/g, '"').replace(/\\\\/g, '\\').replace(/\\n/g, '\n');
      if (/[\u4e00-\u9fff]/.test(key) && !hasTranslation(key)) missing.add(key);
    }
  }
  assert.deepEqual([...missing], [], `untranslated keys: ${[...missing].join(' | ')}`);
});

test('generated names shown through t(variable) are translated', () => {
  const values: string[] = [];
  for (const group of starPetGroups) values.push(group.name, group.description);
  for (const mood of starPetMoods) values.push(mood.name, mood.description);
  for (const color of starPetColors) values.push(color.name);
  for (const slot of growthSlots) values.push(slot.name);
  for (const task of growthCatalog.tasks) values.push(task.name, task.description);
  for (const action of growthCatalog.actions) values.push(action.name, action.message);
  for (const item of growthCatalog.items) values.push(item.name, item.description);
  for (const achievement of growthCatalog.achievements) values.push(achievement.name, achievement.description, achievement.reward);
  for (const preset of appearancePresets) values.push(preset.name);
  for (const direction of gradientDirections) values.push(direction.label);
  for (const source of Object.values(sourceNames)) values.push(source);
  for (const label of Object.values(petStatusLabel)) values.push(label);
  for (const [, label] of languages) values.push(label);
  values.push(defaultCodeCardStyle.watermark);
  for (const text of shareCatalog.texts) values.push(text.label);
  for (const image of shareCatalog.images) values.push(image.label, image.description);
  for (const paper of paperCatalog) values.push(paper.name);
  for (const file of ['src/components/OnboardingGuide.vue', 'src/components/UiColor.vue', 'src/components/StarPetGrowth.vue']) {
    const source = fs.readFileSync(file, 'utf8');
    for (const match of source.matchAll(/'((?:\\.|[^'\\])*)'/g)) {
      const value = match[1].replace(/\\'/g, "'").replace(/\\\\/g, '\\');
      if (/[\u4e00-\u9fff]/.test(value)) values.push(value);
    }
  }
  const missing = [...new Set(values.filter(value => /[\u4e00-\u9fff]/.test(value) && !hasTranslation(value)))];
  assert.deepEqual(missing, [], `untranslated generated names: ${missing.join(' | ')}`);
});
