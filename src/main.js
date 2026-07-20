import { invoke } from '@tauri-apps/api/core';

// ===== elements =====
const searchInput = document.getElementById('searchInput');
const promptBar = document.querySelector('.prompt-bar');
const promptLabel = document.querySelector('.prompt-label');
const resultsEl = document.getElementById('results');
const resultCountEl = document.getElementById('resultCount');
const dotOfficial = document.getElementById('dotOfficial');
const dotAur = document.getElementById('dotAur');

// ===== caret position (fake caret follows real cursor) =====
const measureCtx = document.createElement('canvas').getContext('2d');

function updateCaret() {
  measureCtx.font = getComputedStyle(searchInput).font;
  const text = searchInput.value.substring(0, searchInput.selectionStart);
  const x = measureCtx.measureText(text).width;
  promptBar.style.setProperty('--caret-x', `${x}px`);
}

['input', 'keyup', 'click', 'focus'].forEach(evt =>
  searchInput.addEventListener(evt, updateCaret)
);

// ===== state =====
let requestSeq = 0;

// Search mode: official -> aur -> both
const SEARCH_MODES = ['official', 'aur', 'both'];
let searchMode = 'official';

function applySearchModeStyle() {
  promptLabel.classList.remove('mode-official', 'mode-aur', 'mode-both');
  promptLabel.classList.add(`mode-${searchMode}`);
}

promptLabel.addEventListener('click', () => {
  const currentIndex = SEARCH_MODES.indexOf(searchMode);
  searchMode = SEARCH_MODES[(currentIndex + 1) % SEARCH_MODES.length];
  applySearchModeStyle();

  // Re-run search immediately if a query is already present.
  if (searchInput.value.trim().length >= 2) {
    doSearch(searchInput.value);
  }
});

applySearchModeStyle();

// ===== helpers =====
function escapeHtml(s) {
  return (s || '').replace(/[&<>"']/g, c => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'
  }[c]));
}

// ===== card templates =====
function officialCard(pkg) {
  const installCmd = `sudo pacman -S ${pkg.pkgname}`;
  const installedBadge = pkg.installed
    ? '<span class="badge badge-installed">installed</span>'
    : '';

  return `
    <div class="card" data-source="official">
      <div class="card-top">
        <div class="card-title-row">
          <span class="pkg-name">${escapeHtml(pkg.pkgname)}</span>
          <span class="pkg-version">${escapeHtml(pkg.pkgver)}-${escapeHtml(pkg.pkgrel)}</span>
          ${installedBadge}
        </div>
        <span class="badge official">${escapeHtml(pkg.repo)}</span>
      </div>
      <div class="pkg-desc">${escapeHtml(pkg.pkgdesc) || 'No description.'}</div>
      <div class="card-bottom">
        <div class="card-actions">
          <div class="pkg-links">
            <a href="https://archlinux.org/packages/${encodeURIComponent(pkg.repo)}/${encodeURIComponent(pkg.pkgname)}/" target="_blank" rel="noopener">Details ↗</a>
          </div>
          <div class="install-row">
            <span class="install-cmd"><span class="flag">sudo pacman -S</span> ${escapeHtml(pkg.pkgname)}</span>
            <button class="copy-btn" data-cmd="${escapeHtml(installCmd)}">copy</button>
          </div>
        </div>
      </div>
    </div>`;
}

function aurCard(pkg) {
  const installCmd = `yay -S ${pkg.pkgname}`;
  const installedBadge = pkg.installed
    ? '<span class="badge badge-installed">installed</span>'
    : '';

  return `
    <div class="card" data-source="aur">
      <div class="card-top">
        <div class="card-title-row">
          <span class="pkg-name">${escapeHtml(pkg.pkgname)}</span>
          <span class="pkg-version">${escapeHtml(pkg.pkgver)}-${escapeHtml(pkg.pkgrel)}</span>
          ${installedBadge}
        </div>
        <span class="badge aur">${escapeHtml(pkg.repo)}</span>
      </div>
      <div class="pkg-desc">${escapeHtml(pkg.pkgdesc) || 'No description.'}</div>
      <div class="card-bottom">
        <div class="card-actions">
          <div class="pkg-links">
            <a href="https://aur.archlinux.org/packages/${encodeURIComponent(pkg.pkgname)}" target="_blank" rel="noopener">PKGBUILD ↗</a>
          </div>
          <div class="install-row">
            <span class="install-cmd"><span class="flag">yay -S</span> ${escapeHtml(pkg.pkgname)}</span>
            <button class="copy-btn" data-cmd="${escapeHtml(installCmd)}">copy</button>
          </div>
        </div>
      </div>
    </div>`;
}

// ===== fetching =====
async function fetchOfficial(q) {
  dotOfficial.className = 'dot loading';
  try {
    const packages = await invoke('search_official', { query: q });
    dotOfficial.className = 'dot up';
    return (packages || []).sort((a, b) => a.pkgname.localeCompare(b.pkgname));
  } catch (e) {
    console.error('search_official failed:', e);
    dotOfficial.className = 'dot down';
    return [];
  }
}

async function fetchAur(q) {
  dotAur.className = 'dot loading';
  try {
    const packages = await invoke('search_aur', { query: q });
    dotAur.className = 'dot up';
    return (packages || []).sort((a, b) => a.pkgname.localeCompare(b.pkgname));
  } catch (e) {
    console.error('search_aur failed:', e);
    dotAur.className = 'dot down';
    return [];
  }
}

// ===== rendering =====
function renderEmpty(msg, sub) {
  resultsEl.innerHTML = `
    <div class="empty-state">
      <div class="big">$ _</div>
      ${msg}${sub ? `<br><span class="empty-sub">${sub}</span>` : ''}
    </div>`;
}

async function doSearch(query) {
  const q = query.trim();
  const mySeq = ++requestSeq;

  if (q.length === 0) {
    renderEmpty(
      'Search for a package — for example <b class="hl">htop</b>, <b class="hl">firefox</b> or <b class="hl">yay</b>'
    );
    resultCountEl.textContent = '';
    dotOfficial.className = 'dot';
    dotAur.className = 'dot';
    return;
  }

  if (q.length < 2) {
    renderEmpty('Enter at least 2 characters');
    return;
  }

  resultsEl.innerHTML = `<div class="empty-state">Searching for "${escapeHtml(q)}"<span class="cursor-blink"></span></div>`;

  const wantOfficial = searchMode === 'official' || searchMode === 'both';
  const wantAur = searchMode === 'aur' || searchMode === 'both';

  const [official, aur] = await Promise.all([
    wantOfficial ? fetchOfficial(q) : Promise.resolve([]),
    wantAur ? fetchAur(q) : Promise.resolve([]),
  ]);
  if (mySeq !== requestSeq) return; // stale response, a newer search is in flight

  if (!wantOfficial) dotOfficial.className = 'dot';
  if (!wantAur) dotAur.className = 'dot';

  if (official.length + aur.length === 0) {
    renderEmpty(`No packages found for "${escapeHtml(q)}"`, 'Check the package name or try another search.');
    resultCountEl.textContent = '';
    return;
  }

  resultCountEl.textContent = `${official.length} Official · ${aur.length} AUR`;
  resultsEl.innerHTML = official.map(officialCard).join('') + aur.map(aurCard).join('');
}

// ===== event wiring =====
searchInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    doSearch(searchInput.value);
  }
});

document.querySelectorAll('.quick-tag').forEach(tag => {
  tag.addEventListener('click', () => {
    searchInput.value = tag.textContent;
    doSearch(tag.textContent);
  });
});

resultsEl.addEventListener('click', async (e) => {
  const btn = e.target.closest('.copy-btn');
  if (!btn) return;

  await navigator.clipboard.writeText(btn.dataset.cmd);
  btn.textContent = 'copied';
  btn.classList.add('copied');

  setTimeout(() => {
    btn.textContent = 'copy';
    btn.classList.remove('copied');
  }, 1200);
});

// ===== init =====
updateCaret();
doSearch('');