import test from 'node:test';
import assert from 'node:assert/strict';
import { markdownCode, formatJson, safeFilename } from '../src/snippets.ts';
test('Markdown fence handles embedded fences and title newlines', () => {
  assert.equal(markdownCode('标题\n换行', 'const text = "```";', 'typescript'), '# 标题 换行\n\n````typescript\nconst text = "```";\n````\n');
});
test('export metadata cannot inject fences or escape directories', () => {
  assert.equal(markdownCode('x', 'x', 'js\n```'), '# x\n\n```\nx\n```\n');
  assert.equal(safeFilename('../../foo:bar'), '..-..-foo-bar');
});
test('JSON formatting preserves values and rejects broken input', () => {
  assert.equal(formatJson('{"star":1}'), '{\n  "star": 1\n}');
  assert.throws(() => formatJson('{broken'));
});
