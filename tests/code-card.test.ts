import test from 'node:test';
import assert from 'node:assert/strict';
import { codeCardLayout, defaultCodeCardStyle } from '../src/code-card.ts';

test('code card layout preserves Chinese lines, applies padding, and rejects oversize output', () => {
  const measure = (text: string) => [...text].reduce((width, ch) => width + (/[^\x00-\x7f]/.test(ch) ? 15 : 9), 0);
  const style = { ...defaultCodeCardStyle, paddingX: 48, paddingY: 64 };
  const layout = codeCardLayout('const 星星 = "你好";\r\n\t// 第二行', style, measure);
  assert.deepEqual(layout.lines, ['const 星星 = "你好";', '  // 第二行']);
  assert.equal(layout.width, layout.cardWidth + 96);
  assert.equal(layout.height, layout.cardHeight + 128 + 25);
  const plain = codeCardLayout('星'.repeat(120), { ...style, lineNumbers: false, watermark: '' }, measure);
  assert.equal(plain.gutter, 0);
  assert.equal(plain.cardWidth, 120 * 15 + 56);
  assert.equal(plain.height, plain.cardHeight + 128);
  assert.throws(() => codeCardLayout('星'.repeat(121), style, measure), /120/);
  assert.throws(() => codeCardLayout('\n'.repeat(100), style, measure), /100/);
  assert.equal(codeCardLayout('x', { ...style, paddingX: -1, paddingY: 999 }, measure).paddingX, 16);
});

const segmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' });
const measure = (text: string) => [...segmenter.segment(text)].reduce((width, part) => width + (part.segment.codePointAt(0)! > 127 ? 20 : 10), 0);

test('fixed width soft-wraps Chinese, compound emoji and combining marks without changing source text', () => {
  const source = '星空'.repeat(10) + '👩🏽‍🚀e\u0301' + '🌟'.repeat(12);
  const style = { ...defaultCodeCardStyle, widthMode: 'fixed' as const, fixedWidth: 320 };
  const layout = codeCardLayout(source, style, measure);
  assert.equal(layout.cardWidth, 320);
  assert.equal(layout.text, source);
  assert.deepEqual(layout.lines, [source]);
  assert.equal(layout.rows.map(row => row.text).join(''), source);
  assert.ok(layout.rows.length > 1);
  const boundaries = new Set([...segmenter.segment(source)].map(part => part.index).concat(source.length));
  for (const row of layout.rows) {
    assert.ok(measure(row.text) <= layout.contentWidth);
    assert.ok(boundaries.has(row.start));
    assert.ok(boundaries.has(row.start + row.text.length));
    assert.equal(source.slice(row.start, row.start + row.text.length), row.text);
  }
  assert.deepEqual(layout.rows.map(row => row.lineNumber), [1, ...Array(layout.rows.length - 1).fill(null)]);
  assert.deepEqual(codeCardLayout(source, { ...style, codeTheme: 'light', palette: 'solid' }, measure), layout);
});

test('wrapped layout keeps empty logical lines and numbers only the first visual row of each line', () => {
  const source = '\n' + '星'.repeat(24) + '\n\n';
  const style = { ...defaultCodeCardStyle, widthMode: 'fixed' as const, fixedWidth: 320 };
  const layout = codeCardLayout(source, style, measure);
  assert.deepEqual(layout.lines, ['', '星'.repeat(24), '', '']);
  assert.deepEqual(layout.rows.filter(row => row.lineNumber !== null).map(row => row.lineNumber), [1, 2, 3, 4]);
  assert.deepEqual(layout.rows.filter(row => row.text === '').map(row => row.lineNumber), [1, 3, 4]);
  assert.ok(layout.rows.some(row => row.lineNumber === null));
  for (const row of layout.rows) assert.equal(source.slice(row.start, row.start + row.text.length), row.text);
  const withoutNumbers = codeCardLayout(source, { ...style, lineNumbers: false }, measure);
  assert.equal(withoutNumbers.gutter, 0);
  assert.ok(withoutNumbers.contentWidth > layout.contentWidth);
  assert.equal(layout.cardHeight, 57 + layout.rows.length * 25 + 28);
  assert.equal(codeCardLayout('', style, measure).rows[0]?.lineNumber, 1);
});

test('fixed widths are bounded and soft wrapping cannot allocate an oversized 2x bitmap', () => {
  const style = { ...defaultCodeCardStyle, widthMode: 'fixed' as const };
  assert.equal(codeCardLayout('hello', { ...style, fixedWidth: -5 }, measure).cardWidth, 320);
  assert.equal(codeCardLayout('hello', { ...style, fixedWidth: 10000 }, measure).cardWidth, 960);
  assert.throws(() => codeCardLayout('👩🏽‍🚀', { ...style, fixedWidth: 320 }, text => text.includes('👩') ? 300 : measure(text)), /完整字符/);
  const manyLines = Array(100).fill('星'.repeat(120)).join('\n');
  assert.throws(() => codeCardLayout(manyLines, { ...style, fixedWidth: 320 }, measure), /尺寸过大/);
  assert.throws(() => codeCardLayout(manyLines, { ...style, fixedWidth: 960 }, measure), /尺寸过大/);
  assert.throws(() => codeCardLayout('wide', defaultCodeCardStyle, () => 100000), /尺寸过大/);
});
