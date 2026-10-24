export interface CodeCardStyle {
  palette: 'night' | 'nebula' | 'solid';
  background: string;
  paddingX: number;
  paddingY: number;
  shadow: boolean;
  windowStyle: 'mac' | 'minimal';
  lineNumbers: boolean;
  watermark: string;
  watermarkOpacity: number;
}

export const defaultCodeCardStyle: CodeCardStyle = {
  palette: 'night', background: '#24324b', paddingX: 32, paddingY: 32,
  shadow: true, windowStyle: 'mac', lineNumbers: true,
  watermark: '✦ 星笺 STARCLIP · 拾起每一次灵感', watermarkOpacity: 0.65,
};

export function codeCardLayout(content: string, style: CodeCardStyle, measure: (text: string) => number) {
  const text = content.replace(/\r\n?/g, '\n').replace(/\t/g, '  ');
  const lines = text.split('\n');
  if (lines.length > 100 || lines.some(line => [...line].length > 120)) {
    throw new Error('代码卡片最多 100 行，每行最多 120 个字符。完整内容仍可保存或导出 Markdown。');
  }
  const paddingX = Math.round(Math.min(96, Math.max(16, Number(style.paddingX) || 16)));
  const paddingY = Math.round(Math.min(96, Math.max(16, Number(style.paddingY) || 16)));
  const gutter = style.lineNumbers ? Math.ceil(measure(String(lines.length))) + 22 : 0;
  const cardWidth = Math.max(480, Math.ceil(Math.max(0, ...lines.map(measure))) + gutter + 56);
  const cardHeight = Math.max(170, 57 + lines.length * 25 + 28);
  const watermarkHeight = style.watermark.trim() ? 25 : 0;
  return { text, lines, paddingX, paddingY, gutter, cardWidth, cardHeight, width: cardWidth + paddingX * 2, height: cardHeight + paddingY * 2 + watermarkHeight };
}

export function renderCodeCard(title: string, content: string, language: string, highlighted: string, style: CodeCardStyle) {
  const canvas = document.createElement('canvas');
  const context = canvas.getContext('2d');
  if (!context) throw new Error('当前环境无法生成卡片预览');
  const font = '15px Menlo, Consolas, "PingFang SC", monospace';
  context.font = font;
  const layout = codeCardLayout(content, style, text => context.measureText(text).width);
  const { width, height, paddingX: px, paddingY: py, cardWidth, cardHeight, gutter } = layout;
  canvas.width = width * 2;
  canvas.height = height * 2;
  context.scale(2, 2);
  if (style.palette === 'solid') {
    context.fillStyle = /^#[\da-f]{6}$/i.test(style.background) ? style.background : '#24324b';
  } else {
    const gradient = context.createLinearGradient(0, 0, width, height);
    gradient.addColorStop(0, style.palette === 'night' ? '#263c57' : '#593a5c');
    gradient.addColorStop(1, style.palette === 'night' ? '#101925' : '#211829');
    context.fillStyle = gradient;
  }
  context.fillRect(0, 0, width, height);
  if (style.palette !== 'solid') {
    for (let i = 0; i < 52; i++) {
      context.fillStyle = i % 3 === 0 ? '#e9c98b70' : '#a1b7d44a';
      context.beginPath(); context.arc((i * 139 + 17) % width, (i * 73 + 12) % height, i % 4 === 0 ? 1.3 : 0.7, 0, Math.PI * 2); context.fill();
    }
  }
  context.save();
  if (style.shadow) { context.shadowColor = '#0009'; context.shadowBlur = 26; context.shadowOffsetY = 9; }
  context.fillStyle = '#0e141f'; context.beginPath(); context.roundRect(px, py, cardWidth, cardHeight, 12); context.fill();
  context.restore();
  context.strokeStyle = '#c7d9ff18'; context.lineWidth = 1;
  context.beginPath(); context.roundRect(px + 0.5, py + 0.5, cardWidth - 1, cardHeight - 1, 12); context.stroke();

  const headerY = py + 26;
  let titleX = px + 25;
  if (style.windowStyle === 'mac') {
    ['#ef7a7a', '#e3bf79', '#8fc2a4'].forEach((color, i) => {
      context.fillStyle = color; context.beginPath(); context.arc(px + 25 + i * 17, headerY - 4, 4.5, 0, Math.PI * 2); context.fill();
    });
    titleX += 63;
  }
  context.font = '12px -apple-system, "PingFang SC", sans-serif';
  const languageWidth = context.measureText(language).width;
  const titleWidth = px + cardWidth - 46 - languageWidth - titleX;
  let shortTitle = title.replace(/[\r\n]+/g, ' ').trim() || '一页星码';
  while (context.measureText(shortTitle).width > titleWidth && shortTitle.length > 1) shortTitle = [...shortTitle].slice(0, -2).join('') + '…';
  context.fillStyle = '#c2cbda'; context.fillText(shortTitle, titleX, headerY);
  context.fillStyle = '#75859e'; context.textAlign = 'right'; context.fillText(language, px + cardWidth - 24, headerY); context.textAlign = 'left';
  context.strokeStyle = '#b8caff10'; context.beginPath(); context.moveTo(px + 1, py + 44); context.lineTo(px + cardWidth - 1, py + 44); context.stroke();

  // Both the visible preview and PNG use this exact canvas, including font metrics and token colors.
  context.save();
  context.beginPath(); context.rect(px + 1, py + 45, cardWidth - 2, cardHeight - 46); context.clip();
  context.font = font;
  const left = px + 28 + gutter;
  const top = py + 73;
  const colors: Record<string, string> = { keyword: '#be9de3', string: '#a8c99b', comment: '#6d7b92', number: '#e3bf79', title: '#88bccc', built_in: '#d2af85', attr: '#88bccc', literal: '#d8a1bc', type: '#88bccc', params: '#d8e0ec', variable: '#d8a1bc', tag: '#be9de3', name: '#88bccc' };
  const tokens = new DOMParser().parseFromString(`<pre><span>${highlighted}</span></pre>`, 'text/html').querySelector('pre')!;
  let x = left, row = 0;
  function render(node: Node, color = '#d8e0ec') {
    if (node.nodeType === Node.TEXT_NODE) {
      const parts = (node.textContent || '').replace(/\r\n?/g, '\n').replace(/\t/g, '  ').split('\n');
      parts.forEach((part, index) => {
        if (index) { row++; x = left; }
        context!.fillStyle = color; context!.fillText(part, x, top + row * 25); x += context!.measureText(part).width;
      });
    } else {
      const element = node as Element;
      const token = [...(element.classList || [])].find(name => name.startsWith('hljs-'))?.slice(5);
      node.childNodes.forEach(child => render(child, token && colors[token] ? colors[token]! : color));
    }
  }
  render(tokens);
  if (style.lineNumbers) {
    context.fillStyle = '#4f607a'; context.textAlign = 'right';
    layout.lines.forEach((_, index) => context.fillText(String(index + 1), left - 20, top + index * 25));
  }
  context.restore();
  if (style.watermark.trim()) {
    context.globalAlpha = Math.min(1, Math.max(0, style.watermarkOpacity));
    context.font = '12px -apple-system, "PingFang SC", sans-serif'; context.textAlign = 'center'; context.fillStyle = '#e0d4be';
    context.fillText(style.watermark.replace(/[\r\n]+/g, ' ').slice(0, 80), width / 2, height - Math.max(12, py / 2), cardWidth);
    context.globalAlpha = 1;
  }
  return { url: canvas.toDataURL('image/png'), width: canvas.width, height: canvas.height };
}
