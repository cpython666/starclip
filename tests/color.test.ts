import assert from 'node:assert/strict';
import test from 'node:test';
import { normalizeHex } from '../src/color.ts';

test('custom color input accepts any full RGB hex without accepting partial or CSS values', () => {
  for (const value of ['#12aBf0', '12ABF0', '  #12abf0  ']) assert.equal(normalizeHex(value), '#12abf0');
  assert.equal(normalizeHex('#000000'), '#000000');
  assert.equal(normalizeHex('#ffffff'), '#ffffff');
  for (const value of ['', '#fff', '#12345678', '#12x456', 'red', 'rgb(1,2,3)', '#１２３４５６', '#12 3456']) {
    assert.equal(normalizeHex(value), null, value);
  }
});
