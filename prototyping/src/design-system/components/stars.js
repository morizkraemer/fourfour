import './stars.css';

/**
 * Stars (rating) primitive — §2.7
 * 0–5 rating display & input using filled star glyphs.
 */

const NS = 'http://www.w3.org/2000/svg';
const STAR_PATH = 'M11.48 3.5a.6.6 0 0 1 1.04 0l2.45 4.97 5.48.8a.6.6 0 0 1 .33 1.02l-3.96 3.86.94 5.46a.6.6 0 0 1-.87.63L12 17.98l-4.9 2.57a.6.6 0 0 1-.87-.63l.94-5.46-3.97-3.86a.6.6 0 0 1 .33-1.02l5.48-.8z';

function createStarSVG() {
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('class', 'ff-stars__star');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', '12');
  svg.setAttribute('height', '12');
  svg.setAttribute('fill', 'currentColor');
  svg.setAttribute('stroke', 'none');

  const path = document.createElementNS(NS, 'path');
  path.setAttribute('d', STAR_PATH);
  svg.appendChild(path);

  return svg;
}

export function createStars({ rating = 0, interactive = false, onChange } = {}) {
  let current = Math.max(0, Math.min(5, Math.round(rating)));

  const stars = Array.from({ length: 5 }, () => createStarSVG());

  const root = document.createElement('div');
  root.className = 'ff-stars' + (interactive ? ' ff-stars--interactive' : '');
  stars.forEach((s) => root.appendChild(s));

  function applyRating() {
    stars.forEach((svg, i) => {
      svg.classList.toggle('ff-stars__star--filled', i < current);
    });
  }

  function setRating(n) {
    current = Math.max(0, Math.min(5, Math.round(n)));
    applyRating();
  }

  if (interactive) {
    root.addEventListener('click', (e) => {
      const star = e.target.closest('.ff-stars__star');
      if (!star) return;
      const idx = stars.indexOf(star);
      if (idx === -1) return;
      const next = idx + 1;
      setRating(next === current ? 0 : next);
      if (typeof onChange === 'function') onChange(current);
    });
  }

  applyRating();

  return { element: root, setRating };
}
