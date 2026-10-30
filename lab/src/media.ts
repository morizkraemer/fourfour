import { type MediaItem, isVideoUrl, mediaCaption } from './model';

export function el<K extends keyof HTMLElementTagNameMap>(tag: K, className?: string, text?: string): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

let viewer: HTMLElement | null = null;
let autoplay: IntersectionObserver | null = null;

function visibility(): IntersectionObserver {
  autoplay ??= new IntersectionObserver(
    (entries) => {
      for (const e of entries) {
        const v = e.target as HTMLVideoElement;
        if (e.isIntersecting) void v.play().catch(() => undefined);
        else v.pause();
      }
    },
    { threshold: 0.35 },
  );
  return autoplay;
}

function mediaNode(item: MediaItem, alt: string, eager: boolean, inViewer: boolean): HTMLElement {
  if (isVideoUrl(item.url)) {
    const v = el('video');
    v.src = item.url;
    v.muted = true;
    v.loop = true;
    v.playsInline = true;
    v.preload = eager ? 'auto' : 'metadata';
    if (inViewer) {
      v.controls = true;
      v.autoplay = true;
    } else visibility().observe(v);
    return v;
  }
  const img = el('img');
  img.alt = alt;
  img.decoding = 'async';
  img.loading = eager ? 'eager' : 'lazy';
  img.src = item.url;
  return img;
}

function closeViewer(): void {
  viewer?.remove();
  viewer = null;
  document.body.classList.remove('viewerOpen');
}

function openViewer(items: readonly MediaItem[], start: number, alt: string): void {
  closeViewer();
  const box = el('div', 'mvBox');
  box.setAttribute('role', 'dialog');
  const strip = el('div', 'mvStrip');
  const caption = el('div', 'mvCap');
  items.forEach((item) => {
    const slide = el('div', 'mvSlide');
    slide.append(mediaNode(item, alt, true, true));
    strip.append(slide);
  });
  const close = el('button', 'mvClose', 'Close');
  close.addEventListener('click', closeViewer);
  const setCap = (): void => {
    const i = Math.round(strip.scrollLeft / Math.max(1, strip.clientWidth));
    caption.textContent = items.length > 1 ? `${mediaCaption(items[i] ?? items[0]!, i, items.length)}  (${i + 1}/${items.length})` : '';
  };
  strip.addEventListener('scroll', setCap, { passive: true });
  box.append(strip, caption, close);
  document.body.append(box);
  document.body.classList.add('viewerOpen');
  viewer = box;
  strip.scrollLeft = start * strip.clientWidth;
  setCap();
  const onKey = (e: KeyboardEvent): void => {
    if (e.key !== 'Escape') return;
    closeViewer();
    window.removeEventListener('keydown', onKey);
  };
  window.addEventListener('keydown', onKey);
}

export function mediaStrip(items: readonly MediaItem[], alt: string, eagerFirst: boolean): HTMLElement {
  const wrap = el('div', 'mediaWrap');
  const strip = el('div', 'mediaStrip');
  const chips = el('div', 'chips');
  items.forEach((item, i) => {
    const slide = el('button', 'mediaSlide');
    slide.type = 'button';
    slide.setAttribute('aria-label', `View ${mediaCaption(item, i, items.length) || alt} full screen`);
    slide.append(mediaNode(item, alt, eagerFirst && i === 0, false));
    slide.addEventListener('click', () => openViewer(items, i, alt));
    strip.append(slide);
    if (items.length > 1) {
      const chip = el('button', i === 0 ? 'chip on' : 'chip', mediaCaption(item, i, items.length));
      chip.type = 'button';
      chip.addEventListener('click', () => strip.scrollTo({ left: i * strip.clientWidth, behavior: 'smooth' }));
      chips.append(chip);
    }
  });
  wrap.append(strip);
  if (items.length > 1) {
    strip.addEventListener(
      'scroll',
      () => {
        const at = Math.round(strip.scrollLeft / Math.max(1, strip.clientWidth));
        chips.querySelectorAll('.chip').forEach((c, i) => c.classList.toggle('on', i === at));
      },
      { passive: true },
    );
    wrap.append(chips);
  }
  return wrap;
}
