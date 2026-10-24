export interface CodeCardStyle {
  palette: 'night' | 'nebula' | 'solid';
  codeTheme: 'dark' | 'light';
  widthMode: 'auto' | 'fixed';
  fixedWidth: number;
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
  codeTheme: 'dark', widthMode: 'auto', fixedWidth: 640,
  shadow: true, windowStyle: 'mac', lineNumbers: true,
  watermark: '✦ 星笺 STARCLIP · 拾起每一次灵感', watermarkOpacity: 0.65,
};

const graphemes = new Intl.Segmenter(undefined, { granularity: 'grapheme' });

export function codeCardLayout(content: string, style: CodeCardStyle, measure: (text: string) => number) {
  const text = content.replace(/\r\n?/g, '\n').replace(/\t/g, '  ');
  const lines = text.split('\n');
  if (lines.length > 100 || lines.some(line => [...line].length > 120)) {
    throw new Error('代码卡片最多 100 行，每行最多 120 个字符。完整内容仍可保存或导出 Markdown。');
  }
  const paddingX = Math.round(Math.min(96, Math.max(16, Number(style.paddingX) || 16)));
  const paddingY = Math.round(Math.min(96, Math.max(16, Number(style.paddingY) || 16)));
  const gutter = style.lineNumbers ? Math.ceil(measure(String(lines.length))) + 22 : 0;
  const cardWidth = style.widthMode === 'fixed'
    ? Math.round(Math.min(960, Math.max(320, Number(style.fixedWidth) || 640)))
    : Math.max(480, Math.ceil(Math.max(0, ...lines.map(measure))) + gutter + 56);
  const contentWidth = cardWidth - gutter - 56;
  const rows: Array<{ text: string; start: number; lineNumber: number | null }> = [];
  let offset = 0;
  lines.forEach((line, index) => {
    let row = '', start = 0;
    for (const part of graphemes.segment(line)) {
      if (style.widthMode === 'fixed' && measure(part.segment) > contentWidth) {
        throw new Error('有完整字符超出代码窗宽度，请增大固定宽度或改为自适应正文。');
      }
      if (style.widthMode === 'fixed' && row && measure(row + part.segment) > contentWidth) {
        rows.push({ text: row, start: offset + start, lineNumber: start === 0 ? index + 1 : null });
        row = ''; start = part.index;
      }
      row += part.segment;
    }
    rows.push({ text: row, start: offset + start, lineNumber: start === 0 ? index + 1 : null });
    offset += line.length + 1;
  });
  const cardHeight = Math.max(170, 57 + rows.length * 25 + 28);
  const watermarkHeight = style.watermark.trim() ? 25 : 0;
  const width = cardWidth + paddingX * 2, height = cardHeight + paddingY * 2 + watermarkHeight;
  // Bound the actual 2× bitmap before allocating it, including extra rows from soft wrapping.
  if (!Number.isFinite(width * height) || width * 2 > 16384 || height * 2 > 16384 || width * height * 4 > 24000000) {
    throw new Error('卡片尺寸过大，请增加固定宽度或减少内容后再导出。');
  }
  return { text, lines, rows, contentWidth, paddingX, paddingY, gutter, cardWidth, cardHeight, width, height };
}

function fitText(text: string, width: number, measure: (text: string) => number) {
  if (measure(text) <= width) return text;
  const parts = [...graphemes.segment(text)].map(part => part.segment);
  while (parts.length && measure(parts.join('') + '…') > width) parts.pop();
  return measure('…') <= width ? parts.join('') + '…' : '';
}

export function renderCodeCard(title: string, content: string, language: string, highlighted: string, style: CodeCardStyle) {
  const canvas = document.createElement('canvas');
  const context = canvas.getContext('2d');
  if (!context) throw new Error('当前环境无法生成卡片预览');
  const font = '15px Menlo, Consolas, "PingFang SC", monospace';
  context.font = font;
  const layout = codeCardLayout(content, style, text => context.measureText(text).width);
  const { width, height, paddingX: px, paddingY: py, cardWidth, cardHeight, gutter } = layout;
  const theme = style.codeTheme === 'light'
    ? { background: '#f8f6ef', text: '#24314a', title: '#31415b', muted: '#65738a', border: '#28364a22',
      colors: { keyword: '#7848a3', string: '#387044', comment: '#65738a', number: '#956015', title: '#176887', built_in: '#895723', attr: '#176887', literal: '#a03f68', type: '#176887', params: '#24314a', variable: '#a03f68', tag: '#7848a3', name: '#176887' } }
    : { background: '#0e141f', text: '#d8e0ec', title: '#c2cbda', muted: '#8190a7', border: '#c7d9ff18',
      colors: { keyword: '#be9de3', string: '#a8c99b', comment: '#8290a6', number: '#e3bf79', title: '#88bccc', built_in: '#d2af85', attr: '#88bccc', literal: '#d8a1bc', type: '#88bccc', params: '#d8e0ec', variable: '#d8a1bc', tag: '#be9de3', name: '#88bccc' } };
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
  context.fillStyle = theme.background; context.beginPath(); context.roundRect(px, py, cardWidth, cardHeight, 12); context.fill();
  context.restore();
  context.strokeStyle = theme.border; context.lineWidth = 1;
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
  const shortTitle = fitText(title.replace(/[\r\n]+/g, ' ').trim() || '一页星码', titleWidth, text => context.measureText(text).width);
  context.fillStyle = theme.title; context.fillText(shortTitle, titleX, headerY);
  context.fillStyle = theme.muted; context.textAlign = 'right'; context.fillText(language, px + cardWidth - 24, headerY); context.textAlign = 'left';
  context.strokeStyle = theme.border; context.beginPath(); context.moveTo(px + 1, py + 44); context.lineTo(px + cardWidth - 1, py + 44); context.stroke();

  // Both the visible preview and PNG use this exact canvas, including font metrics and token colors.
  context.save();
  context.beginPath(); context.rect(px + 1, py + 45, cardWidth - 2, cardHeight - 46); context.clip();
  context.font = font;
  const left = px + 28 + gutter;
  const top = py + 73;
  const colors: Record<string, string> = theme.colors;
  const tokens = new DOMParser().parseFromString(`<pre><span>${highlighted}</span></pre>`, 'text/html').querySelector('pre')!;
  const tokenColors: string[] = [];
  let highlightedText = '';
  function readColors(node: Node, color = theme.text) {
    if (node.nodeType === Node.TEXT_NODE) {
      const value = (node.textContent || '').replace(/\r\n?/g, '\n').replace(/\t/g, '  ');
      highlightedText += value;
      for (let index = 0; index < value.length; index++) tokenColors.push(color);
    } else {
      const element = node as Element;
      const token = [...(element.classList || [])].find(name => name.startsWith('hljs-'))?.slice(5);
      node.childNodes.forEach(child => readColors(child, token && colors[token] ? colors[token]! : color));
    }
  }
  readColors(tokens);
  if (highlightedText !== layout.text) tokenColors.length = 0;
  layout.rows.forEach((row, index) => {
    let run = '', start = 0, color = theme.text;
    function paint() {
      context!.fillStyle = color;
      context!.fillText(run, left + context!.measureText(row.text.slice(0, start)).width, top + index * 25);
    }
    // Keep complete graphemes together even when a highlighter splits token boundaries.
    for (const part of graphemes.segment(row.text)) {
      const nextColor = tokenColors[row.start + part.index] || theme.text;
      if (run && nextColor !== color) { paint(); run = ''; start = part.index; }
      color = nextColor; run += part.segment;
    }
    if (run) paint();
  });
  if (style.lineNumbers) {
    context.fillStyle = theme.muted; context.textAlign = 'right';
    layout.rows.forEach((row, index) => { if (row.lineNumber !== null) context.fillText(String(row.lineNumber), left - 20, top + index * 25); });
  }
  context.restore();
  if (style.watermark.trim()) {
    context.globalAlpha = Math.min(1, Math.max(0, style.watermarkOpacity));
    const background = /^#[\da-f]{6}$/i.test(style.background) ? style.background : '#24324b';
    const channels = [1, 3, 5].map(start => Number.parseInt(background.slice(start, start + 2), 16));
    const lightBackground = style.palette === 'solid' && channels[0]! * 0.299 + channels[1]! * 0.587 + channels[2]! * 0.114 > 160;
    context.font = '12px -apple-system, "PingFang SC", sans-serif'; context.textAlign = 'center'; context.fillStyle = lightBackground ? '#25344e' : '#e0d4be';
    const watermark = [...graphemes.segment(style.watermark.replace(/[\r\n]+/g, ' '))].slice(0, 80).map(part => part.segment).join('');
    context.fillText(fitText(watermark, cardWidth, text => context.measureText(text).width), width / 2, height - Math.max(12, py / 2));
    context.globalAlpha = 1;
  }
  return { url: canvas.toDataURL('image/png'), width: canvas.width, height: canvas.height };
}
