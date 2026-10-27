const ELLIPSIS = '…';
const TAIL_SHARE = 0.6;

export function middleTruncate(text, maxChars) {
  const chars = Array.from(text);
  if (chars.length <= maxChars) return text;
  if (maxChars <= 1) return ELLIPSIS;
  const budget = maxChars - 1;
  const lastSep = Math.max(text.lastIndexOf('/'), text.lastIndexOf('\\'));
  const nameLen = lastSep < 0 ? 0 : Array.from(text.slice(lastSep)).length;
  const tailLen = Math.min(budget, Math.max(nameLen, Math.ceil(budget * TAIL_SHARE)));
  const headLen = budget - tailLen;
  return chars.slice(0, headLen).join('') + ELLIPSIS + chars.slice(chars.length - tailLen).join('');
}
