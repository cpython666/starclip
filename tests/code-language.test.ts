import test from 'node:test';
import assert from 'node:assert/strict';
import { detectLanguage } from '../src/code-language.ts';
import { languages } from '../src/snippets.ts';

const samples: [string, string][] = [
  ['rust', 'fn main() {\n    println!("Hello, stars. ✦");\n}'],
  ['rust', 'pub struct Star { name: String }\nimpl Star {\n    pub fn shine(&self) -> &str { &self.name }\n}'],
  ['go', 'package main\n\nimport "fmt"\n\nfunc main() {\n\tmsg := "hi"\n\tfmt.Println(msg)\n}'],
  ['java', 'public class Main {\n    public static void main(String[] args) {\n        System.out.println("hi");\n    }\n}'],
  ['python', 'def shine(star):\n    if star is None:\n        return False\n    print(f"{star} ✦")\n    return True'],
  ['python', 'import os\n\nclass Sky:\n    def __init__(self):\n        self.stars = []'],
  ['javascript', 'const stars = [1, 2, 3];\nstars.forEach((star) => console.log(star));'],
  ['javascript', "import { ref } from 'vue';\nexport function useSky() {\n  return ref(0);\n}"],
  ['typescript', 'interface Star {\n  name: string;\n  size: number;\n}\nconst sun: Star = { name: "Sun", size: 1 };'],
  ['typescript', "export function shine(name: string): void {\n  console.log(name);\n}"],
  ['json', '{\n  "star": 1,\n  "names": ["Vega"]\n}'],
  ['html', '<!DOCTYPE html>\n<html>\n  <body><p class="star">✦</p></body>\n</html>'],
  ['html', '<div class="sky">\n  <span>Vega</span>\n</div>'],
  ['css', '.star {\n  color: #fff;\n  margin: 4px;\n}'],
  ['css', ':root {\n  --sky: #101828;\n}\nbody { background: var(--sky); }'],
  ['sql', 'SELECT name, size FROM stars WHERE size > 3 ORDER BY name;'],
  ['sql', 'CREATE TABLE stars (id INTEGER PRIMARY KEY, name TEXT);'],
  ['bash', '#!/usr/bin/env bash\nset -e\necho "hi"'],
  ['bash', 'npm install\ngit add . && git commit -m "stars"'],
  ['markdown', '# 星笺\n\n- 收藏代码\n- 制作 [星码卡](https://example.com)\n\n```rust\nfn main() {}\n```'],
];

test('common snippets are recognized as their language', () => {
  for (const [expected, code] of samples) assert.equal(detectLanguage(code), expected, code);
});

test('prose, empty text and weak hints stay plain text', () => {
  for (const text of ['', '   \n', '让散落的灵感，\n在这里连成星座。', 'Meet me at 5, then we can look at the stars.', 'hello world', '{broken']) {
    assert.equal(detectLanguage(text), 'plaintext', text);
  }
});

test('detected languages are always selectable options', () => {
  const keys = new Set<string>(languages.map(([key]) => key));
  for (const [, code] of samples) assert.ok(keys.has(detectLanguage(code)));
});
