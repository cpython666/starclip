import { languages } from './snippets.ts';

export type CodeLanguage = (typeof languages)[number][0];
type Rule = [pattern: RegExp, weight: number];

// Each rule adds its weight once when it matches; the strongest language wins.
// Local heuristics keep detection instant and offline, and stay predictable for short snippets.
const rules: Partial<Record<CodeLanguage, Rule[]>> = {
  rust: [
    [/\bfn\s+\w+\s*[(<]/, 3], [/\blet\s+mut\b/, 3], [/\b(println|print|format|vec|panic|assert(_eq)?)!\s*[([]/, 3],
    [/\bpub\s+(fn|struct|enum|mod|trait)\b/, 3], [/\bimpl(\s*<[^>]*>)?\s+\w+/, 2], [/&(mut\s+)?self\b/, 3],
    [/^\s*use\s+\w+(::[\w{}*, ]+)+;/m, 3], [/\bmatch\s+[\w.&]+\s*\{/, 2], [/::\w+/, 1], [/->\s*[\w<(&]/, 1],
    [/\b(Option|Result|Vec|String)<|\bSome\(|\bOk\(|\bErr\(/, 2],
  ],
  go: [
    [/^\s*package\s+\w+\s*$/m, 4], [/\bfunc\s+(\(\s*\w+\s+\*?\w+\s*\)\s*)?\w+\s*\(/, 3], [/\w\s*:=\s*/, 2],
    [/\bfmt\.\w+\(/, 3], [/^\s*import\s+\(/m, 3], [/\berr\s*!=\s*nil\b/, 3], [/\bgo\s+func\b|\bchan\s+\w+|\bdefer\s+\w/, 2],
  ],
  java: [
    [/\bpublic\s+(static\s+)?(final\s+)?(class|interface|enum|void)\b/, 3], [/\bSystem\.(out|err)\.print/, 4],
    [/^\s*import\s+(static\s+)?java[x]?\.[\w.*]+;/m, 4], [/^\s*package\s+[\w.]+;/m, 4], [/@Override\b/, 3],
    [/\bString\[\]\s+\w+/, 4], [/\b(private|protected)\s+(static\s+)?(final\s+)?[A-Z]?\w+(<[^>]*>)?(\[\])?\s+\w+\s*[;=(]/, 2],
    [/\bnew\s+[A-Z]\w*(<[^>]*>)?\(/, 1],
  ],
  python: [
    [/^\s*(async\s+)?def\s+\w+\s*\(.*\)\s*(->\s*[^:]+)?:\s*$/m, 4], [/^\s*class\s+\w+(\(.*\))?:\s*$/m, 3],
    [/^\s*(from\s+[\w.]+\s+)?import\s+[\w.]+(\s+as\s+\w+)?(,\s*[\w.]+)*\s*$/m, 2], [/\bself\.\w+/, 2],
    [/^\s*(if|elif|for|while|with|try|except|else|finally)\b[^{};\n]*:\s*$/m, 2], [/\b(None|True|False)\b/, 1],
    [/__\w+__/, 2], [/\bprint\(/, 1], [/^\s*elif\b|\bexcept\s+\w+|\blambda\s+\w*:|\bf["']/m, 2],
  ],
  javascript: [
    [/\b(const|let|var)\s+[\w{}[\], ]+\s*=/, 2], [/=>/, 1], [/\bconsole\.\w+\(/, 3], [/\bfunction\s*\*?\s*\w*\s*\(/, 3],
    [/\brequire\(\s*['"]/, 3], [/^\s*import\s+[^;\n]*\s+from\s+['"]/m, 3], [/^\s*export\s+(default\s+)?(async\s+)?(function|const|class|let)\b/m, 3],
    [/\b(document|window)\.\w+/, 2], [/===|!==/, 2], [/\bawait\s+\w|\.then\(/, 1],
  ],
  typescript: [
    [/\binterface\s+\w+(<[^>]*>)?\s*(extends\s+[\w<>, ]+)?\{/, 3], [/^\s*(export\s+)?type\s+\w+(<[^>]*>)?\s*=/m, 3],
    [/[\w)?]\s*:\s*(string|number|boolean|any|void|unknown|never|Record<|Promise<|Array<)/, 3],
    [/\bas\s+(const|string|number|any|unknown)\b/, 2], [/\b(private|public|readonly)\s+\w+\s*[?:]/, 2], [/\bimport\s+type\b/, 3],
    [/<\w+(\[\])?>\s*\(/, 1],
  ],
  html: [
    [/^\s*<!doctype\s+html/i, 6], [/<(html|head|body|div|span|p|a|ul|ol|li|table|script|style|meta|link|section|button|input|img|template|form|h[1-6])\b[^>]*>/i, 3],
    [/<\/[a-z][\w-]*>/i, 2], [/<\w+[^>]*\s(class|id|href|src)="[^"]*"/, 2],
  ],
  css: [
    [/^\s*([.#]?[\w-]+|\*|:root|@media[^{]*)[^{}\n;]*\{\s*$/m, 1], [/^\s*-?[a-z-]+\s*:\s*[^;{}]+;\s*$/m, 2],
    [/@(media|import|keyframes|font-face|supports)\b|!important/, 3], [/\d(px|rem|em|vh|vw)\b/, 2],
    [/#[0-9a-f]{3,8}\b\s*;/i, 2], [/var\(--[\w-]+\)|^\s*--[\w-]+\s*:/m, 3],
    [/\b(color|margin|padding|display|font-size|background|border|width|height)\s*:/, 1],
  ],
  sql: [
    [/\bselect\s+(distinct\s+)?[\w*.,()\s]+?\s+from\s+[\w.\"`]/i, 4], [/\b(insert\s+into|update\s+\w+\s+set|delete\s+from|create\s+(table|index|view)|alter\s+table|drop\s+table)\b/i, 4],
    [/\b(WHERE|JOIN|GROUP BY|ORDER BY|LIMIT|VALUES|PRIMARY KEY)\b/, 2], [/;\s*$/m, 1],
  ],
  bash: [
    [/^#!.*\b(ba|z|da)?sh\b/m, 6],
    [/^\s*(sudo\s+)?(echo|cd|ls|export|apt(-get)?|brew|npm|npx|pnpm|yarn|git|curl|wget|mkdir|rm|cp|mv|chmod|chown|cargo|pip3?|docker|source|grep|cat)\s/m, 3],
    [/^\s*(fi|done|esac)\s*$/m, 3], [/^\s*(if|while)\s+\[\[?\s/m, 3], [/\$\{?\w+\}?/, 1], [/\s(&&|\|\|)\s|\s\|\s\w/, 1], [/^\s*\$\s+\w/m, 2],
    [/\s--?[a-z][\w-]*/, 1],
  ],
  markdown: [
    [/^#{1,6}\s+\S/m, 1], [/^```/m, 4], [/^\s*[-*+]\s+\S/m, 1], [/^\s*\d+\.\s+\S/m, 1], [/\[[^\]\n]+\]\([^)\s]+\)/, 3],
    [/\*\*[^*\n]+\*\*/, 2], [/^>\s/m, 1], [/^\|.*\|\s*$/m, 1], [/^\s*[-|: ]{3,}\s*$/m, 1],
  ],
};

const minimumScore = 3;

export function detectLanguage(content: string): CodeLanguage {
  const text = content.slice(0, 20000).trim();
  if (!text) return 'plaintext';
  if (/^[{[]/.test(text)) { try { JSON.parse(text); return 'json'; } catch { /* not JSON, keep scoring */ } }
  const scores = new Map<CodeLanguage, number>();
  for (const [language, list] of Object.entries(rules) as [CodeLanguage, Rule[]][]) {
    scores.set(language, list.reduce((sum, [pattern, weight]) => sum + (pattern.test(text) ? weight : 0), 0));
  }
  // TypeScript is a superset: its own markers decide, on top of everything JavaScript-like.
  const typed = scores.get('typescript') ?? 0;
  scores.set('typescript', typed ? typed + (scores.get('javascript') ?? 0) : 0);
  let best: CodeLanguage = 'plaintext', bestScore = minimumScore - 1;
  for (const [language, score] of scores) if (score > bestScore) { best = language; bestScore = score; }
  return best;
}
