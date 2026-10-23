export const languages = [
  ['plaintext', '纯文本'], ['javascript', 'JavaScript'], ['typescript', 'TypeScript'],
  ['python', 'Python'], ['rust', 'Rust'], ['json', 'JSON'], ['html', 'HTML'],
  ['css', 'CSS'], ['bash', 'Shell'], ['sql', 'SQL'], ['markdown', 'Markdown'],
  ['go', 'Go'], ['java', 'Java'],
] as const;

export function markdownCode(title: string, content: string, language: string): string {
  const runs = content.match(/`+/g) ?? [];
  const fence = '`'.repeat(Math.max(3, ...runs.map((run) => run.length + 1)));
  const safeLanguage = languages.some(([key]) => key === language) ? language : 'plaintext';
  return `# ${title.replace(/[\r\n]+/g, ' ')}\n\n${fence}${safeLanguage === 'plaintext' ? '' : safeLanguage}\n${content}\n${fence}\n`;
}

export function formatJson(content: string): string { return JSON.stringify(JSON.parse(content), null, 2); }

export function download(data: Blob, filename: string): void {
  const url = URL.createObjectURL(data);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

export function safeFilename(title: string): string {
  return title.replace(/[<>:"/\\|?*\u0000-\u001f]/g, '-').slice(0, 64).trim() || 'starclip-snippet';
}
