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
