'use strict';

// The box's screen: sign-in, browsing the music library, the player. Everything talks to
// ownbox on localhost; ownbox talks to the own.audio server.

const $ = (id) => document.getElementById(id);

// --- small helpers ------------------------------------------------------------------

/** Builds an element. Text always goes in as text, never as HTML: titles come from files. */
function el(tag, attrs, ...children) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v === undefined || v === null || v === false) continue;
    if (k === 'class') node.className = v;
    else if (k === 'text') node.textContent = v;
    else if (k.startsWith('on')) node.addEventListener(k.slice(2), v);
    else node.setAttribute(k, v === true ? '' : v);
  }
  for (const c of children.flat()) {
    if (c === null || c === undefined || c === false) continue;
    node.append(c instanceof Node ? c : document.createTextNode(String(c)));
  }
  return node;
}

// The own.audio mark, as on www.own.audio (audio2-www src/lib/mark.ts, the detailed
// variant): the violet core, two arcs, and the family of five around it.
const FAMILY = ['#FABB05', '#29ABE2', '#FF3B30', '#FFCC00', '#34C759'];
const MARK = `<circle cx="50" cy="50" r="11" fill="#6E44FF" class="bass-core"/>
<g class="spin-arcs" fill="none">
<path d="M 35 62 A 20 20 0 0 0 65 62" stroke="#48484a" stroke-width="3.5" stroke-linecap="round"/>
<path d="M 28 70 A 30 30 0 0 0 72 70" stroke="#48484a" stroke-width="2.5" stroke-linecap="round" opacity=".6"/>
</g>
<g class="dance-family">${FAMILY.map((fill, i) => `<g transform="rotate(${i * 72}, 50, 50)">
<circle cx="50" cy="14" r="5" fill="${fill}"/><circle cx="41.5" cy="15" r="2.5" fill="${fill}"/><circle cx="58.5" cy="15" r="2.5" fill="${fill}"/></g>`).join('')}</g>`;

/** The mark with "own.audio" and "box" beside it. Animated only where it is the whole
 * screen's greeting (the wizard): a moving picture costs the Pi's CPU everywhere else. */
function brand(size, { animated = false, big = false } = {}) {
  const svg = `<svg width="${size}" height="${size}" viewBox="0 0 100 100" role="img" aria-label="own.audio" class="own-mark${animated ? ' own-mark--animated' : ''}">${MARK}</svg>`;
  const lockup = el('div', { class: big ? 'brand-lockup big' : 'brand-lockup' });
  lockup.innerHTML = svg;
  lockup.append(el('div', { class: 'brand-words' },
    el('span', { class: 'wordmark' }, 'own', el('span', { class: 'dot', text: '.' }), 'audio'),
    el('span', { class: 'brand-sub', text: 'box' })));
  return lockup;
}

const ICONS = {
  play: '<path d="M8 5v14l11-7z"/>',
  pause: '<path d="M6 5h4v14H6zM14 5h4v14h-4z"/>',
  prev: '<path d="M6 6h2v12H6zM9.5 12l8.5 6V6z"/>',
  next: '<path d="M16 6h2v12h-2zM6 18l8.5-6L6 6z"/>',
  back: '<path d="M15.5 4.5 8 12l7.5 7.5" fill="none" stroke="currentColor" stroke-width="2.6"/>',
  down: '<path d="M4.5 8.5 12 16l7.5-7.5" fill="none" stroke="currentColor" stroke-width="2.6"/>',
  settings: '<path d="M4 7h9M17 7h3M4 17h3M11 17h9" fill="none" stroke="currentColor" stroke-width="2.2"/><circle cx="15" cy="7" r="2.4"/><circle cx="9" cy="17" r="2.4"/>',
  queue: '<path d="M4 6h16M4 12h16M4 18h10" fill="none" stroke="currentColor" stroke-width="2.2"/>',
  enqueue: '<path d="M4 6h12M4 12h12M4 18h7M18 14v7M14.5 17.5h7" fill="none" stroke="currentColor" stroke-width="2.2"/>',
  playlist: '<path d="M4 6h10M4 11h10M4 16h6" fill="none" stroke="currentColor" stroke-width="2.2"/><path d="M19 4v10.5a2.5 2.5 0 1 1-2-2.45V4z"/>',
  eq: '<path d="M6 4v16M12 4v16M18 4v16" fill="none" stroke="currentColor" stroke-width="2" opacity=".45"/><rect x="3.5" y="12" width="5" height="3" rx="1.5"/><rect x="9.5" y="6" width="5" height="3" rx="1.5"/><rect x="15.5" y="14" width="5" height="3" rx="1.5"/>',
  sleep: '<path d="M20 14.5A8.5 8.5 0 1 1 9.5 4a7 7 0 0 0 10.5 10.5z"/>',
  volume: '<path d="M4 9v6h4l5 4V5L8 9z"/><path d="M16 8.5a5 5 0 0 1 0 7M18.5 6a8.5 8.5 0 0 1 0 12" fill="none" stroke="currentColor" stroke-width="2"/>',
};

function icon(name) {
  return `<svg viewBox="0 0 24 24" aria-hidden="true">${ICONS[name]}</svg>`;
}

/**
 * An action button (play, queue, playlist…) carries both an icon and its label; Settings →
 * Buttons shows one or the other through body[data-buttons], so switching redraws nothing.
 * The label stays the button's name for screen readers and the hover tip either way.
 */
function actify(node, name, label) {
  if (!node.classList.contains('act')) {
    node.classList.add('act');
    node.replaceChildren(el('span', { class: 'label' }));
  }
  if (node.dataset.icon !== name) {
    node.querySelector('svg')?.remove();
    node.insertAdjacentHTML('afterbegin', icon(name));
    node.dataset.icon = name;
  }
  node.querySelector('.label').textContent = label;
  node.title = label;
  node.setAttribute('aria-label', label);
  return node;
}

/** The player's own action buttons; their labels change with the language. */
function labelPlayer() {
  actify($('np-eq'), 'eq', t('Equalizer'));
  actify($('np-add'), 'playlist', t('To a playlist'));
  if (!$('np-queue').classList.contains('act')) actify($('np-queue'), 'queue', t('Queue'));
  if (!$('np-sleep').classList.contains('act')) actify($('np-sleep'), 'sleep', t('Sleep timer'));
}

function act(name, label, cls, onclick) {
  return actify(el('button', { class: cls, onclick }), name, label);
}

function setIcon(node, name) {
  if (node.dataset.icon !== name) {
    node.innerHTML = icon(name);
    node.dataset.icon = name;
  }
}

function clock(secs) {
  secs = Math.max(0, Math.round(secs || 0));
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = String(secs % 60).padStart(2, '0');
  return h ? `${h}:${String(m).padStart(2, '0')}:${s}` : `${m}:${s}`;
}

function cover(src, cls) {
  return src ? el('img', { src, class: cls, loading: 'lazy', alt: '' }) : el('div', { class: cls ? `${cls} noimg` : 'noimg' });
}

class ApiError extends Error {
  constructor(message, status) {
    super(message);
    this.status = status;
  }
}

// The button pressed last, so a POST it starts can mark it busy until the box answers.
let pressed = { button: null, at: 0 };
document.addEventListener('click', (e) => {
  const b = e.target.closest && e.target.closest('button');
  if (b) pressed = { button: b, at: Date.now() };
}, true);
// Safari on iOS draws :active only when the page listens for touches.
document.addEventListener('touchstart', () => {}, { passive: true });
// A quick tap is over before :active is drawn: `pressing` keeps the push visible for at
// least a moment, then lets the button spring back.
document.addEventListener('pointerdown', (e) => {
  const node = e.target.closest && e.target.closest('button, .tile, .row');
  if (!node) return;
  const at = performance.now();
  node.classList.add('pressing');
  const up = () => {
    setTimeout(() => node.classList.remove('pressing'), Math.max(0, 140 - (performance.now() - at)));
    window.removeEventListener('pointerup', up, true);
    window.removeEventListener('pointercancel', up, true);
  };
  window.addEventListener('pointerup', up, true);
  window.addEventListener('pointercancel', up, true);
}, true);

async function call(method, path, body) {
  const busy = method !== 'GET' && Date.now() - pressed.at < 300 ? pressed.button : null;
  if (busy) busy.classList.add('busy');
  try {
    return await request(method, path, body);
  } finally {
    if (busy) busy.classList.remove('busy');
  }
}

/** How long a request may take before the page stops waiting: an update downloads the
 * program, a server check may wait on a slow address, everything else is quick. */
function requestTimeout(path) {
  if (path.startsWith('/api/update')) return 300000;
  if (path.startsWith('/api/setup/server') || path.startsWith('/api/sign-in') || path.startsWith('/api/wifi')) return 60000;
  return 15000;
}

async function request(method, path, body) {
  let r;
  const abort = new AbortController();
  const timer = setTimeout(() => abort.abort(), requestTimeout(path));
  try {
    r = await fetch(path, {
      method,
      cache: 'no-store',
      headers: body ? { 'Content-Type': 'application/json' } : undefined,
      body: body ? JSON.stringify(body) : undefined,
      signal: abort.signal,
    });
  } catch (e) {
    throw new ApiError('ownbox is not answering', 0);
  } finally {
    clearTimeout(timer);
  }
  const data = await r.json().catch(() => null);
  if (!r.ok) throw new ApiError((data && data.error) || `HTTP ${r.status}`, r.status);
  return data;
}
/** `fn` for a timer: a call still waiting (a slow box, a sleeping phone) is not joined by
 * another, so they do not pile up. */
function once(fn) {
  let running = false;
  return async (...args) => {
    if (running) return;
    running = true;
    try { await fn(...args); } finally { running = false; }
  };
}

const get = (path) => call('GET', path);
const post = (path, body) => call('POST', path, body || {});
const del = (path) => call('DELETE', path);

// --- sign-in ----------------------------------------------------------------------

let shownCode = null;
let signedIn = null;

function renderSession(s) {
  if (!setup.done) return; // the first-run wizard owns the screen until it is finished
  const now = s.state === 'signed_in';
  $('app').hidden = !now;
  $('signin').hidden = now;
  if (!now) $('nowplaying').hidden = true;
  if (now !== signedIn) {
    signedIn = now;
    if (now) go('music');
  }
  if (now) {
    account = s.user;
    shownCode = null;
    // What plays goes on (mpv has the stream); only browsing needs the server.
    $('offline').hidden = s.server_reachable !== false;
    $('offline').textContent = t('The own.audio server cannot be reached. What is playing goes on; the library comes back with the server.');
    return;
  }
  if (REMOTE) {
    $('signin-pending').hidden = true;
    $('signin-start').hidden = false;
    $('signin-button').hidden = true;
    $('signin-error').textContent = t('The box is not signed in. Sign it in on its own screen.');
    return;
  }
  const pending = s.state === 'pending';
  $('signin-start').hidden = pending;
  $('signin-pending').hidden = !pending;
  if (pending) {
    if (shownCode !== s.user_code) {
      shownCode = s.user_code;
      // The query string only defeats caching; the server draws the QR for the current code.
      $('qr').src = `/api/sign-in/qr.svg?c=${encodeURIComponent(s.user_code)}`;
    }
    $('qr').hidden = !s.link || HEADLESS;
    showLink($('link'), s.link);
    $('pending-hint').textContent = s.link
      ? t(HEADLESS ? 'Open the address below and confirm the code in your own.audio account.' : 'Scan the code with your phone, or open the address below and enter the code.')
      : t('In the own.audio app or web console, go to "Link a device" and enter the code.');
    $('user-code').textContent = s.user_code;
    $('expires').textContent = t('Valid for {t}', { t: clock(s.expires_in) });
  } else {
    shownCode = null;
    $('signin-error').textContent = s.error || '';
  }
}

let account = null;

/** The sign-in address: shown short on the box's screen; a link to tap on a phone that is
 * the screen of a box without one (it cannot scan itself). */
function showLink(where, link) {
  const short = link ? link.replace(/^https?:\/\//, '').replace(/\?.*$/, '') : '';
  if (HEADLESS && link) {
    where.replaceChildren(el('a', { href: link, target: '_blank', rel: 'noopener', text: short }));
  } else {
    where.textContent = short;
  }
}

// Failed polls in a row: one is a blip (a phone waking, Wi-Fi roaming), not a reason to
// take the page away — and never while the wizard is on screen.
let sessionMisses = 0;

async function refreshSession() {
  try {
    renderSession(await get('/api/session'));
    sessionMisses = 0;
  } catch (e) {
    sessionMisses += 1;
    if (!setup.done || sessionMisses < 3) return;
    $('signin').hidden = false;
    $('app').hidden = true;
    $('signin-error').textContent = t(e.message);
  }
}

async function startSignIn() {
  $('signin-error').textContent = '';
  $('signin-button').disabled = true;
  try {
    await post('/api/sign-in');
  } catch (e) {
    $('signin-error').textContent = e.message;
  }
  $('signin-button').disabled = false;
  refreshSession();
}

$('signin-button').addEventListener('click', startSignIn);
$('new-code').addEventListener('click', startSignIn);

// --- navigation -------------------------------------------------------------------
// Each tab is a stack of views; Back pops it. A view is {render(container)}.

let stack = [];
let currentTab = null;

const ROOTS = {
  music: () => musicSection('home'),
  books: () => booksSection('home'),
  podcasts: () => podcastsSection('home'),
  search: () => searchView(),
  settings: () => settingsView(),
};

function comingView(text) {
  return { async render(box) { box.append(el('div', { class: 'message', text })); } };
}

// Music has its own row of sections under the top bar.
const MUSIC = [['home', 'Home'], ['albums', 'Albums'], ['artists', 'Artists'], ['playlists', 'Playlists']];

function musicSection(name, opts = {}) {
  const inner = {
    home: () => musicHome(),
    albums: () => albumsView(opts.sort || 'added', opts.from),
    artists: () => artistsView(opts.from),
    playlists: () => playlistsView(),
  }[name]();
  return {
    async render(box) {
      box.append(el('div', { class: 'subnav' }, MUSIC.map(([key, label]) => el('button', {
        class: key === name ? 'on' : '',
        text: t(label),
        onclick: () => { stack = [musicSection(key)]; render(); },
      }))));
      await inner.render(box);
    },
  };
}

function shelf(title, onAll, content) {
  return el('div', {},
    el('div', { class: 'shelf-head' },
      el('h3', { text: title }),
      onAll ? el('button', { class: 'secondary', text: t('See all'), onclick: onAll }) : null),
    content);
}

function musicHome() {
  return {
    async render(box) {
      const [recent, favourites, lists, played] = await Promise.all([
        get('/api/library/albums?sort=added'),
        get('/api/library/favourites').catch(() => []),
        get('/api/library/playlists').catch(() => []),
        get('/api/library/recent-albums').catch(() => []),
      ]);
      // Playlists first: the ones played on this box latest, then smart, then by name.
      const ordered = [...lists].sort((a, b) => (b.played_at || 0) - (a.played_at || 0)
        || Number(b.smart) - Number(a.smart) || a.name.localeCompare(b.name));
      if (ordered.length) {
        box.append(shelf(t('Playlists'), () => { stack = [musicSection('playlists')]; render(); },
          el('div', { class: 'hscroll' }, ordered.slice(0, 24).map(playlistTile))));
      }
      if (played.length) {
        box.append(shelf(t('Recently played'), null, el('div', { class: 'hscroll' }, played.map(albumTile))));
      }
      box.append(shelf(t('Recently added'), () => { stack = [musicSection('albums', { sort: 'added' })]; render(); },
        el('div', { class: 'hscroll' }, recent.items.slice(0, 24).map(albumTile))));
      if (favourites.length) {
        const rows = el('div', { class: 'rows' },
          favourites.slice(0, 5).map((t, i) => trackRow(t, i + 1, () => play({ tracks: favourites, start: i }), artistAndAlbum(t))));
        box.append(shelf(t('Favourites'), () => push(favouritesView()), rows));
      }
    },
  };
}

/** A playlist as a tile: its cover (or its first track's), or the music tint with a mark. */
function playlistTile(p) {
  return el('div', { class: 'tile', onclick: () => push(p.smart ? smartPlaylistView(p) : playlistView(p)) },
    p.image ? cover(p.image) : el('div', { class: 'noimg tint', text: p.smart ? '✦' : '♪' }),
    el('div', { class: 't ellipsis', text: p.name }),
    el('div', { class: 'a ellipsis muted', text: p.smart ? t('Smart playlist') : tn(p.track_count, 'tracks') }));
}

function favouritesView() {
  return {
    async render(box) {
      const tracks = await get('/api/library/favourites');
      box.append(el('div', { class: 'toolbar' },
        el('h2', { text: t('Favourites') }),
        el('div', { class: 'spacer' }),
        tracks.length ? act('play', t('Play'), 'primary', () => play({ tracks, start: 0 })) : null));
      const rows = el('div', { class: 'rows' });
      tracks.forEach((t, i) => rows.append(trackRow(t, i + 1, () => play({ tracks, start: i }), artistAndAlbum(t))));
      box.append(rows);
      if (!tracks.length) box.append(el('div', { class: 'message', text: t('No favourites yet. Mark songs with a heart in the own.audio app.') }));
    },
  };
}

function go(tab) {
  currentTab = tab;
  stack = [ROOTS[tab]()];
  render();
}

function push(view) {
  stack.push(view);
  render();
}

function back() {
  if (stack.length > 1) {
    stack.pop();
    render();
  }
}

let renderToken = 0;

async function render() {
  const token = ++renderToken;
  if (currentTab !== 'search' || stack.length > 1) document.body.classList.remove('typing');
  // Each library has its own colour, as in the web app; the rest takes music's.
  document.body.dataset.section = currentTab === 'books' || currentTab === 'podcasts' ? currentTab : 'music';
  const view = stack[stack.length - 1];
  $('back').hidden = stack.length < 2;
  document.querySelectorAll('[data-tab]').forEach((b) => b.classList.toggle('active', b.dataset.tab === currentTab));
  const box = $('view');
  hideScrub();
  box.replaceChildren(el('div', { class: 'message', text: t('Loading…') }));
  box.scrollTop = 0;
  const content = el('div');
  try {
    await view.render(content);
    if (token !== renderToken) return; // the user went elsewhere meanwhile
    box.replaceChildren(content);
    if (view.scrollTop) box.scrollTop = view.scrollTop;
    markCurrent();
  } catch (e) {
    if (token !== renderToken) return;
    if (e.status === 401) return refreshSession();
    box.replaceChildren(el('div', { class: 'message' },
      el('div', { text: t("Can't load this: {e}", { e: t(e.message) }) }),
      el('button', { class: 'secondary', text: t('Try again'), onclick: render })));
  }
}

$('view').addEventListener('scroll', () => {
  const view = stack[stack.length - 1];
  if (view) view.scrollTop = $('view').scrollTop;
});

setIcon($('back'), 'back');
$('back').addEventListener('click', back);
setIcon(document.querySelector('[data-tab="settings"]'), 'settings');
document.querySelectorAll('[data-tab]').forEach((b) => b.addEventListener('click', () => go(b.dataset.tab)));

/** Loads the next page when the sentinel scrolls into view. */
function pager(container, load) {
  const sentinel = el('div', { class: 'more', text: t('Loading…') });
  let busy = false;
  const observer = new IntersectionObserver(async (entries) => {
    if (!entries.some((e) => e.isIntersecting) || busy) return;
    busy = true;
    try {
      const more = await load();
      if (!more) {
        observer.disconnect();
        sentinel.remove();
      }
    } catch (e) {
      sentinel.textContent = t("Can't load more: {e}", { e: e.message });
      observer.disconnect();
    }
    busy = false;
  }, { root: $('view'), rootMargin: '400px' });
  container.append(sentinel);
  observer.observe(sentinel);
}

// --- playing ----------------------------------------------------------------------

async function play(body) {
  try {
    await post('/api/player/play', body);
    pollNow();
  } catch (e) {
    toast(t("Can't play: {e}", { e: t(e.message) }));
  }
}

// --- views ------------------------------------------------------------------------

function albumTile(a) {
  return el('div', { class: 'tile', onclick: () => push(albumView(a.id)) },
    cover(a.image),
    el('div', { class: 't ellipsis', text: a.title }),
    el('div', { class: 'a ellipsis muted', text: a.artist }));
}

// Long lists start at a letter. The server's page cursor is base64url JSON of the last
// row's [sort key, id], where the sort key is the lower-cased name: a cursor of
// [letter, nil id] starts the list at that letter. The server calls its cursors opaque, so
// this leans on how it builds them today (own-audio-foss `v2/page.rs`); if that changes,
// the jump breaks and the lists still work from the top.
const NIL_UUID = '00000000-0000-0000-0000-000000000000';
const LETTERS = [...'ABCDEFGHIJKLMNOPQRSTUVWXYZ'];

function cursorAt(letter) {
  const json = JSON.stringify([letter.toLowerCase(), NIL_UUID]);
  return btoa(unescape(encodeURIComponent(json))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function pickLetter(current, onPick) {
  const close = () => overlay.remove();
  const overlay = el('div', { class: 'letters' },
    el('div', { class: 'letters-grid' },
      el('button', { class: !current ? 'on' : '', text: t('Top'), onclick: () => { close(); onPick(null); } }),
      LETTERS.map((l) => el('button', { class: l === current ? 'on' : '', text: l, onclick: () => { close(); onPick(l); } }))),
    el('button', { class: 'secondary', text: t('Cancel'), onclick: close }));
  document.body.append(overlay);
}

// --- fast scroll, as on the iPod ----------------------------------------------------
// A–Z lists get a strip of letters at the right edge. Dragging along it shows the letter
// under the finger large in the middle; letting go jumps there (the lists come page by
// page from the server, so a jump asks for the list from that letter). Scrolling the list
// fast shows the letter it has reached the same way.

const scrub = { onJump: null, current: null, hideTimer: null, lastY: 0, lastAt: 0 };

/** The first letter a list sorts a name under: A–Z without accents, '#' for the rest. */
function initial(name) {
  const c = (name || '').normalize('NFD').replace(/[\u0300-\u036f]/g, '').trim().charAt(0).toUpperCase();
  return c >= 'A' && c <= 'Z' ? c : '#';
}

function lettered(node, name) {
  node.dataset.letter = initial(name);
  return node;
}

/** Shows the strip for the view being drawn; `render` takes it away for the next one. */
function letterScrub(current, onJump) {
  scrub.onJump = onJump;
  scrub.current = current;
  document.body.classList.add('scrubbing');
  placeScrub();
  $('scrub').hidden = false;
}

function hideScrub() {
  scrub.onJump = null;
  document.body.classList.remove('scrubbing');
  $('scrub').hidden = true;
  $('bigletter').hidden = true;
}

/** Beside the list, above the now-playing bar (which lies over the view's bottom). */
function placeScrub() {
  const r = $('view').getBoundingClientRect();
  const bar = $('nowbar');
  const bottom = bar.hidden ? r.bottom : Math.min(r.bottom, bar.getBoundingClientRect().top);
  const strip = $('scrub');
  strip.style.top = `${r.top + 6}px`;
  strip.style.height = `${Math.max(0, bottom - r.top - 12)}px`;
}

const SCRUB_LETTERS = ['#', ...LETTERS];

function showBig(letter, linger) {
  const big = $('bigletter');
  big.textContent = letter;
  big.hidden = false;
  clearTimeout(scrub.hideTimer);
  if (linger) scrub.hideTimer = setTimeout(() => { big.hidden = true; }, linger);
}

(() => {
  const strip = $('scrub');
  strip.replaceChildren(...SCRUB_LETTERS.map((l) => el('span', { text: l })));
  let picked = null;
  const at = (y) => {
    const r = strip.getBoundingClientRect();
    const i = Math.min(SCRUB_LETTERS.length - 1, Math.max(0, Math.floor(((y - r.top) / r.height) * SCRUB_LETTERS.length)));
    return SCRUB_LETTERS[i];
  };
  const move = (e) => {
    const l = at(e.clientY);
    if (l !== picked && navigator.vibrate) navigator.vibrate(5);
    picked = l;
    showBig(l);
  };
  strip.addEventListener('pointerdown', (e) => {
    e.preventDefault();
    strip.setPointerCapture(e.pointerId);
    strip.classList.add('on');
    move(e);
  });
  strip.addEventListener('pointermove', (e) => { if (strip.hasPointerCapture(e.pointerId)) move(e); });
  const done = () => {
    strip.classList.remove('on');
    $('bigletter').hidden = true;
    if (picked && scrub.onJump) scrub.onJump(picked === '#' ? null : picked);
    picked = null;
  };
  strip.addEventListener('pointerup', done);
  strip.addEventListener('pointercancel', () => { strip.classList.remove('on'); picked = null; $('bigletter').hidden = true; });
  window.addEventListener('resize', () => { if (scrub.onJump) placeScrub(); });

  // Scrolling fast through an A–Z list: the letter at the top of what shows.
  $('view').addEventListener('scroll', () => {
    if (!scrub.onJump) return;
    const view = $('view');
    const now = performance.now();
    const speed = Math.abs(view.scrollTop - scrub.lastY) / Math.max(1, now - scrub.lastAt);
    scrub.lastY = view.scrollTop;
    scrub.lastAt = now;
    if (speed < 1.2 && $('bigletter').hidden) return;
    const r = view.getBoundingClientRect();
    const hit = document.elementFromPoint(r.left + 40, r.top + 40);
    const item = hit && hit.closest('[data-letter]');
    if (item) showBig(item.dataset.letter, 600);
  }, { passive: true });
})();

/** A list read page by page, optionally from a letter; `add` turns items into nodes. */
async function pagedList(box, url, from, list, add) {
  const sep = url.includes('?') ? '&' : '?';
  const first = await get(from ? `${url}${sep}cursor=${cursorAt(from)}` : url);
  first.items.forEach((x) => list.append(add(x)));
  box.append(list);
  let next = first.next;
  if (next) {
    pager(box, async () => {
      const page = await get(`${url}${sep}cursor=${encodeURIComponent(next)}`);
      page.items.forEach((x) => list.append(add(x)));
      next = page.next;
      return Boolean(next);
    });
  }
  return first.items.length;
}

function albumsView(sort, from) {
  return {
    async render(box) {
      const go = (opts) => { stack = [musicSection('albums', opts)]; render(); };
      box.append(el('div', { class: 'toolbar' },
        el('button', { class: sort === 'added' ? 'on' : '', text: t('Recently added'), onclick: () => go({ sort: 'added' }) }),
        el('button', { class: sort === 'title' ? 'on' : '', text: 'A–Z', onclick: () => go({ sort: 'title' }) }),
        el('div', { class: 'spacer' }),
        sort === 'title' ? el('button', { class: 'secondary', text: from ? t('From {l}', { l: from }) : t('Jump to letter'),
          onclick: () => pickLetter(from, (l) => go({ sort: 'title', from: l })) }) : null));
      if (sort === 'title') letterScrub(from, (l) => go({ sort: 'title', from: l }));
      const n = await pagedList(box, `/api/library/albums?sort=${sort}`, sort === 'title' ? from : null, el('div', { class: 'grid' }), (a) => lettered(albumTile(a), a.title));
      if (!n) box.append(el('div', { class: 'message', text: from ? t('Nothing from {l} on.', { l: from }) : 'No albums in this library yet.' }));
    },
  };
}

/** `sub` is the grey second line (artist, album), or nothing. Holding the row opens the
 * track's actions; `more` adds to them (a playlist's own "remove"). */
function trackRow(t, n, onTap, sub, more) {
  // Each song with its small cover, as the web app lists them; an album's own page leaves
  // them out (`image: false`), since they would all be the same.
  const thumb = t.image === false ? null
    : t.image ? el('img', { class: 'thumb', src: t.image, loading: 'lazy', alt: '' })
      : el('div', { class: 'thumb', text: '♪' });
  const row = el('div', { class: 'row', 'data-id': t.id },
    el('div', { class: 'n', text: n }),
    thumb,
    el('div', { class: 'main' },
      el('div', { class: 'ellipsis', text: t.title }),
      sub ? el('div', { class: 'sub ellipsis muted', text: sub }) : null),
    el('div', { class: 'd', text: clock(t.duration_secs) }));
  holdable(row, onTap, () => trackActions(t.image === false ? { ...t, image: null } : t, more));
  return row;
}

/** A tap runs `onTap`; a finger held still for a moment (or a right click) runs `onHold`
 * instead, and the click that follows it is dropped. */
function holdable(node, onTap, onHold) {
  let timer = null;
  let held = false;
  let x = 0;
  let y = 0;
  const cancel = () => { clearTimeout(timer); timer = null; };
  const hold = () => {
    cancel();
    held = true;
    if (navigator.vibrate) navigator.vibrate(15);
    onHold();
  };
  node.addEventListener('pointerdown', (e) => {
    held = false;
    x = e.clientX;
    y = e.clientY;
    cancel();
    timer = setTimeout(hold, 550);
  });
  node.addEventListener('pointermove', (e) => { if (Math.abs(e.clientX - x) + Math.abs(e.clientY - y) > 12) cancel(); });
  ['pointerup', 'pointercancel', 'pointerleave'].forEach((ev) => node.addEventListener(ev, cancel));
  // Android's long press and a right click: the same as holding, never twice.
  node.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    if (!held) hold();
  });
  node.addEventListener('click', (e) => {
    if (held) {
      held = false;
      return;
    }
    if (onTap) onTap(e);
  });
}

const artistAndAlbum = (t) => [t.artist, t.album].filter(Boolean).join(' · ');

function albumView(id) {
  return {
    async render(box) {
      const { album, tracks } = await get(`/api/library/albums/${encodeURIComponent(id)}`);
      const total = tracks.reduce((s, t) => s + (t.duration_secs || 0), 0);
      const facts = [album.year, tn(tracks.length, 'tracks'), clock(total)].filter(Boolean).join(' · ');
      box.append(el('div', { class: 'album-head' },
        cover(album.image),
        el('div', { class: 'info' },
          el('div', { class: 'title clamp2', text: album.title }),
          el('div', { class: 'ellipsis', text: album.artist }),
          el('div', { class: 'muted', text: facts }),
          el('div', { class: 'buttons' },
            act('play', t('Play'), 'primary', () => play({ album_id: id, start: 0 })),
            act('enqueue', t('To the queue'), 'secondary', () => enqueue({ album_id: id }, false)),
            act('playlist', t('To a playlist'), 'secondary', () => openPicker(tracks.map((x) => x.id)))))));
      const discs = new Set(tracks.map((t) => t.disc || 1)).size;
      const rows = el('div', { class: 'rows' });
      let disc = null;
      tracks.forEach((t, i) => {
        if (discs > 1 && (t.disc || 1) !== disc) {
          disc = t.disc || 1;
          rows.append(el('div', { class: 'section-title', text: t('Disc {n}', { n: disc }) }));
        }
        // Within an album only a guest artist is worth a second line.
        const guest = t.artist && t.artist !== album.artist ? t.artist : null;
        rows.append(trackRow({ ...t, image: false }, t.number || i + 1, () => play({ album_id: id, start: i }), guest));
      });
      box.append(rows);
    },
  };
}

function artistsView(from) {
  return {
    async render(box) {
      box.append(el('div', { class: 'toolbar' },
        el('div', { class: 'spacer' }),
        el('button', { class: 'secondary', text: from ? t('From {l}', { l: from }) : t('Jump to letter'),
          onclick: () => pickLetter(from, (l) => { stack = [musicSection('artists', { from: l })]; render(); }) })));
      const row = (a) => el('div', { class: 'row', onclick: () => push(artistView(a)) },
        a.image ? el('img', { class: 'round', src: a.image, loading: 'lazy', alt: '' }) : el('div', { class: 'round noimg', style: 'width:44px;height:44px;border-radius:50%' }),
        el('div', { class: 'main ellipsis', text: a.name }));
      letterScrub(from, (l) => { stack = [musicSection('artists', { from: l })]; render(); });
      const n = await pagedList(box, '/api/library/artists', from, el('div', { class: 'rows' }), (a) => lettered(row(a), a.name));
      if (!n) box.append(el('div', { class: 'message', text: from ? t('Nothing from {l} on.', { l: from }) : 'No artists yet.' }));
    },
  };
}

function artistView(artist) {
  return {
    async render(box) {
      const albums = await get(`/api/library/artists/${encodeURIComponent(artist.id)}/albums`);
      box.append(el('div', { class: 'toolbar' },
        el('h2', { class: 'ellipsis', text: artist.name }),
        el('div', { class: 'spacer' }),
        albums.length ? act('play', t('Play all'), 'primary', () => play({ artist_id: artist.id, start: 0 })) : null));
      const grid = el('div', { class: 'grid' });
      albums.forEach((a) => grid.append(albumTile(a)));
      box.append(grid);
      if (!albums.length) box.append(el('div', { class: 'message', text: t('No albums.') }));
    },
  };
}

function playlistsView() {
  return {
    async render(box) {
      const [lists, presets] = await Promise.all([
        get('/api/library/playlists'),
        get('/api/library/smart-presets').catch(() => []),
      ]);
      box.append(el('div', { class: 'toolbar' },
        el('div', { class: 'spacer' }),
        el('button', { class: 'secondary', text: t('New playlist'), onclick: () => push(playlistName([])) })));
      const listRow = (p) => el('div', { class: 'row', onclick: () => push(p.smart ? smartPlaylistView(p) : playlistView(p)) },
        el('div', { class: 'main' },
          el('div', { class: 'ellipsis', text: p.name }),
          el('div', { class: 'sub muted', text: [p.smart ? t('Smart playlist') : tn(p.track_count, 'tracks'), p.mine ? null : t('shared with you')].filter(Boolean).join(' · ') })));
      // Smart first, as the web app's "Chytré": the saved ones, then the server's presets,
      // which play as they are (a genre station asks for its genre first).
      const smart = lists.filter((p) => p.smart);
      if (smart.length || presets.length) {
        box.append(el('div', { class: 'section-title', text: t('Smart playlists') }),
          el('div', { class: 'rows' }, ...smart.map(listRow), ...presets.map((p) => el('div', {
            class: 'row',
            onclick: () => push(p.slug === 'genre-station' ? genrePicker(p) : presetView(p)),
          }, el('div', { class: 'main' },
            el('div', { class: 'ellipsis', text: presetName(p) }),
            el('div', { class: 'sub muted ellipsis', text: presetAbout(p) }))))));
      }
      const own = lists.filter((p) => !p.smart);
      box.append(el('div', { class: 'section-title', text: t('Playlists') }), el('div', { class: 'rows' }, own.map(listRow)));
      if (!own.length) box.append(el('div', { class: 'message', text: t('No playlists yet.') }));
    },
  };
}

function playlistView(p) {
  return {
    async render(box) {
      const tracks = await get(`/api/library/playlists/${encodeURIComponent(p.id)}`);
      box.append(el('div', { class: 'toolbar' },
        el('h2', { class: 'ellipsis', text: p.name }),
        el('div', { class: 'spacer' }),
        tracks.length ? act('play', t('Play'), 'primary', () => play({ playlist_id: p.id, start: 0 })) : null,
        p.mine ? deletePlaylistButton(p) : null));
      if (p.mine && tracks.length) box.append(el('p', { class: 'muted small', text: t('Hold a track to take it out of the playlist.') }));
      const remove = (track, i) => [t('Remove from playlist'), async () => {
        try {
          await post(`/api/library/playlists/${encodeURIComponent(p.id)}/remove`, { index: i, track_id: track.id });
          toast(t('Removed from {p}', { p: p.name }));
        } catch (e) {
          toast(t(e.message));
        }
        render();
      }];
      const rows = el('div', { class: 'rows' });
      tracks.forEach((track, i) => rows.append(trackRow(track, i + 1, () => play({ playlist_id: p.id, start: i }), artistAndAlbum(track),
        p.mine ? [remove(track, i)] : [])));
      box.append(rows);
      if (!tracks.length) box.append(el('div', { class: 'message', text: t('The playlist is empty. Add tracks from an album, or hold a track anywhere.') }));
    },
  };
}

// A smart playlist is a rule: the server draws its tracks anew at each opening, so what is
// played is exactly the draw on the screen. It is made and changed in the apps.
function smartPlaylistView(p) {
  return {
    async render(box) {
      const tracks = await get(`/api/library/smart-playlists/${encodeURIComponent(p.id)}`);
      box.append(el('div', { class: 'toolbar' },
        el('h2', { class: 'ellipsis', text: p.name }),
        el('div', { class: 'spacer' }),
        tracks.length ? act('play', t('Play'), 'primary', () => play({ tracks, start: 0, from_playlist: p.id })) : null,
        el('button', { class: 'secondary', text: t('New draw'), onclick: () => render() })));
      box.append(el('p', { class: 'muted small', text: t('A smart playlist picks its tracks anew each time it opens.') }));
      const rows = el('div', { class: 'rows' });
      tracks.forEach((track, i) => rows.append(trackRow(track, i + 1, () => play({ tracks, start: i, from_playlist: p.id }), artistAndAlbum(track))));
      box.append(rows);
      if (!tracks.length) box.append(el('div', { class: 'message', text: t('No tracks match this playlist now.') }));
    },
  };
}

// The presets' names and what each picks, as the web app shows them; the server's own
// (English) name for one it adds later.
const PRESETS = {
  'forgotten-favourites': ['Forgotten favourites', "Songs you starred but haven't played for a year."],
  'new-in-the-library': ['New in the library', 'Added in the last 30 days.'],
  'back-in-rotation': ['Back in rotation', 'Rated four stars or more, not heard for two months.'],
  'what-the-family-plays': ['What the family plays', "What others in the family play and you haven't yet."],
  'genre-station': ['Genre station', 'Up to 100 songs of one genre, no artist twice in a row.'],
};
const presetName = (p) => (PRESETS[p.slug] ? t(PRESETS[p.slug][0]) : p.name);
const presetAbout = (p) => (PRESETS[p.slug] ? t(PRESETS[p.slug][1]) : '');

/** A preset drawn now (a genre station with its genre); nothing is saved. */
function presetView(preset, genre) {
  return {
    async render(box) {
      const rule = genre ? { ...preset.rule, filters: { ...preset.rule.filters, genres: { include: genre.tags } } } : preset.rule;
      const tracks = await post('/api/library/smart-resolve', { rule });
      box.append(el('div', { class: 'toolbar' },
        el('h2', { class: 'ellipsis', text: genre ? `${presetName(preset)}: ${genre.name}` : presetName(preset) }),
        el('div', { class: 'spacer' }),
        tracks.length ? act('play', t('Play'), 'primary', () => play({ tracks, start: 0 })) : null,
        el('button', { class: 'secondary', text: t('New draw'), onclick: () => render() })));
      box.append(el('p', { class: 'muted small', text: presetAbout(preset) }));
      const rows = el('div', { class: 'rows' });
      tracks.forEach((track, i) => rows.append(trackRow(track, i + 1, () => play({ tracks, start: i }), artistAndAlbum(track))));
      box.append(rows);
      if (!tracks.length) box.append(el('div', { class: 'message', text: t('No tracks match this playlist now.') }));
    },
  };
}

// Genre tags are a mess in a real library (192 in one 9,000-track library: "Pop, Rock", "Rap/Hip Hop / Jazz",
// "Rock" and "rock", "Musique"), and nobody will tidy them. A genre station is picked
// from a dozen big groups instead. Each tag is split into parts; each part goes to the
// first group whose word it contains (so "Punk Rock" is punk, not rock). A group under
// 20 tracks, and every part no group claims, go to "Other". The station sends the
// server every tag of the group: it matches whole tags, ignoring case.
const GENRE_GROUPS = [
  ['Metal', ['metal', 'thrash', 'doom', 'grindcore']],
  // Two groups, not one: ska first, so "Ska Punk" is ska.
  ['Ska', ['ska']],
  ['Punk', ['punk', 'hardcore', 'oi!']],
  ['Hip-hop & R&B', ['hip hop', 'hip-hop', 'hiphop', 'rap', 'rnb', 'soul', 'funk']],
  ['Electronic & dance', ['electro', 'dance', 'house', 'techno', 'trance', 'edm', 'disco', 'drum', 'dubstep', 'trip-hop', 'trip hop', 'synth', 'ambient', 'club', 'big beat', 'industrial']],
  ['Blues & jazz', ['blues', 'jazz', 'swing', 'boogie', 'big band']],
  ['Folk & country', ['folk', 'country', 'bluegrass', 'americana', 'songwriter', 'acoustic']],
  ['Reggae', ['reggae', 'dancehall', 'dub']],
  ['Classical', ['classical', 'classique', 'klassiek', 'klasick', 'symphon', 'opera', 'baroque', 'orchestra']],
  ['Latin', ['latin', 'latijn', 'salsa', 'reggaeton', 'bossa', 'flamenco', 'espagnol']],
  ['Pop', ['pop', 'top 40', 'schlager', 'chanson', 'vocal', 'oldies']],
  ['Rock', ['rock', 'grunge', 'alternative', 'indie', 'new wave', 'britpop']],
];
// Words that say nothing about the music.
const GENRE_NOISE = ['musique', 'music', 'muziek', 'unknown', 'unknown genre', 'other', 'genre', 'misc', 'audio book', 'audiobook', 'audiobooks', 'spoken word', 'ninguno', 'desconhecido', 'singer'];

function genreGroups(genres) {
  const groups = new Map(GENRE_GROUPS.map(([name]) => [name, { name, tags: new Set(), tracks: 0 }]));
  const other = { name: 'Other', tags: new Set(), tracks: 0, other: true };
  for (const g of genres) {
    // R&B and rock'n'roll keep their ampersand and apostrophes; then , ; / & and " - " split.
    const norm = g.genre.normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase().replace(/r\s*&\s*b|r'n'b|rhythm and blues/g, 'rnb').replace(/rock\s*(&|'n'|and)\s*roll/g, 'rock');
    const parts = norm.split(/\s*(?:[,;/&]|\s-\s)\s*/).map((x) => x.trim()).filter((x) => x && !GENRE_NOISE.includes(x));
    const into = new Set();
    for (const part of parts) {
      const hit = GENRE_GROUPS.find(([, words]) => words.some((w) => part.includes(w)));
      if (hit) into.add(hit[0]);
    }
    if (!parts.length) continue; // only noise: "Unknown", "Musique"
    if (!into.size) into.add('Other');
    for (const name of into) {
      const group = name === 'Other' ? other : groups.get(name);
      group.tags.add(g.genre);
      group.tracks += g.track_count;
    }
  }
  const shown = [...groups.values()].filter((g) => g.tracks >= 20);
  for (const g of groups.values()) {
    if (g.tracks < 20) {
      g.tags.forEach((tag) => other.tags.add(tag));
      other.tracks += g.tracks;
    }
  }
  shown.sort((a, b) => b.tracks - a.tracks);
  if (other.tags.size) shown.push(other);
  return shown;
}

/** The genre for a genre station: a dozen big groups, largest first, then "Other". */
function genrePicker(preset) {
  return {
    async render(box) {
      const groups = genreGroups(await get('/api/library/genres'));
      box.append(el('h2', { text: t('Genre station'), style: 'margin-bottom:6px' }),
        el('p', { class: 'muted small', text: t('Pick a genre.'), style: 'margin-bottom:10px' }));
      box.append(el('div', { class: 'grid genre-grid' }, groups.map((g) => el('button', {
        class: g.other ? 'genre secondary' : 'genre',
        onclick: () => push(presetView(preset, { name: t(g.name), tags: [...g.tags] })),
      }, el('span', { class: 'g', text: t(g.name) }), el('span', { class: 'muted small', text: tn(g.tracks, 'tracks') })))));
    },
  };
}

// Deleting takes two taps, as signing out does.
function deletePlaylistButton(p) {
  let armed = null;
  const b = el('button', { class: 'secondary', text: t('Delete'), onclick: async () => {
    if (!armed) {
      b.textContent = t('Tap again to delete');
      armed = setTimeout(() => { armed = null; b.textContent = t('Delete'); }, 3000);
      return;
    }
    clearTimeout(armed);
    armed = null;
    try {
      await del(`/api/library/playlists/${encodeURIComponent(p.id)}`);
      toast(t('Playlist {p} deleted', { p: p.name }));
      back();
    } catch (e) {
      b.textContent = t('Delete');
      toast(t(e.message));
    }
  } });
  return b;
}

// --- making playlists ---------------------------------------------------------------
// From any track (hold it), an album or the now-playing screen: pick one of the account's
// own playlists, or name a new one. Playlists shared by others are only played.

function trackActions(track, more = []) {
  if (track.book_id || track.podcast_id) return;
  sheet([track.title, track.artist].filter(Boolean).join(' · '), [
    [t('Play next'), () => enqueue({ tracks: [track] }, true)],
    [t('Add to queue'), () => enqueue({ tracks: [track] }, false)],
    [t('Add to playlist'), () => openPicker([track.id])],
    ...more,
  ]);
}

/** After what plays (`next`) or at the end of the queue; with nothing playing, it plays. */
async function enqueue(what, next) {
  try {
    const r = await post('/api/player/enqueue', { ...what, next });
    toast(r.started ? t('Playing') : next ? t('Plays next') : t('Added to the queue'));
  } catch (e) {
    toast(t(e.message));
  }
  pollNow();
}

function openPicker(trackIds) {
  if (!trackIds.length) return;
  $('nowplaying').hidden = true;
  $('queue').hidden = true;
  push(playlistPicker(trackIds));
}

function playlistPicker(trackIds) {
  return {
    async render(box) {
      const lists = (await get('/api/library/playlists')).filter((p) => p.mine && !p.smart);
      box.append(el('h2', { text: trackIds.length > 1 ? t('Add {n} to a playlist', { n: tn(trackIds.length, 'tracks') }) : t('Add to playlist') }),
        el('div', { class: 'buttons', style: 'margin-bottom:12px' },
          el('button', { class: 'primary', text: t('New playlist'), onclick: () => push(playlistName(trackIds)) })));
      const rows = el('div', { class: 'rows' });
      lists.forEach((p) => {
        const row = el('div', { class: 'row', onclick: async () => {
          row.classList.add('busy');
          try {
            await post(`/api/library/playlists/${encodeURIComponent(p.id)}/add`, { track_ids: trackIds });
            toast(t('Added to {p}', { p: p.name }));
            back();
          } catch (e) {
            row.classList.remove('busy');
            toast(t(e.message));
          }
        } },
          el('div', { class: 'main' },
            el('div', { class: 'ellipsis', text: p.name }),
            el('div', { class: 'sub muted', text: tn(p.track_count, 'tracks') })));
        rows.append(row);
      });
      box.append(rows);
      if (!lists.length) box.append(el('div', { class: 'message', text: t('You have no playlists of your own yet.') }));
    },
  };
}

/** Names a new playlist; it starts with `trackIds` (none when made from the list). */
function playlistName(trackIds) {
  return {
    async render(box) {
      const field = { value: '' };
      const text = el('div', { class: 'q' });
      const msg = el('p', { class: 'muted', style: 'min-height:1.4em' });
      const create = async () => {
        if (!field.value.trim()) return;
        msg.className = 'muted';
        msg.textContent = t('Creating…');
        try {
          const p = await post('/api/library/playlists', { name: field.value, track_ids: trackIds });
          toast(trackIds.length ? t('Added to {p}', { p: p.name }) : t('Playlist {p} created', { p: p.name }));
          if (trackIds.length) {
            // Back past the picker to where the tracks were chosen.
            stack.splice(-2);
            render();
          } else {
            stack.pop();
            push(playlistView(p));
          }
        } catch (e) {
          msg.className = 'bad';
          msg.textContent = t(e.message);
        }
      };
      document.body.classList.add('typing');
      box.append(el('div', { class: 'search-wrap' },
        el('h2', { text: t('New playlist') }),
        el('div', { class: 'searchbar' }, text),
        msg,
        el('div', { style: 'flex:1' }),
        textKeyboard(field, { onChange: () => { text.textContent = field.value; }, onDone: create, doneLabel: t('Create') })));
    },
  };
}

/** Actions over the screen, from the bottom; a tap beside them closes them. */
function sheet(title, actions) {
  const close = () => wrap.remove();
  const wrap = el('div', { class: 'sheet-wrap', onclick: (e) => { if (e.target === wrap) close(); } },
    el('div', { class: 'sheet' },
      el('div', { class: 'sheet-title ellipsis', text: title }),
      ...actions.map(([label, run]) => el('button', { class: 'secondary', text: label, onclick: () => { close(); run(); } })),
      el('button', { text: t('Cancel'), onclick: close })));
  document.body.append(wrap);
}

let toastTimer = null;
function toast(text) {
  const box = $('toast');
  box.textContent = text;
  box.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { box.hidden = true; }, 2500);
}

// --- appearance -------------------------------------------------------------------

let THEME = 'dark';

function applyTheme(theme) {
  THEME = theme === 'light' || theme === 'auto' ? theme : 'dark';
  // The box's own screen has no system setting to follow: "as the device" is dark there.
  const kiosk = document.body.classList.contains('kiosk');
  document.documentElement.dataset.theme = kiosk && THEME === 'auto' ? 'dark' : THEME;
}

let BUTTONS = 'text';

function applyButtons(buttons) {
  BUTTONS = buttons === 'icons' ? 'icons' : 'text';
  document.body.dataset.buttons = BUTTONS;
}

function choiceSetting(label, choices, current, set) {
  const box = el('div', { class: 'buttons' });
  const draw = () => box.replaceChildren(...choices.map(([key, text]) => el('button', {
    class: current() === key ? 'on' : 'secondary',
    text: t(text),
    onclick: async () => { await set(key); draw(); },
  })));
  draw();
  return el('div', { class: 'setting' }, el('span', { text: t(label) }), box);
}

function appearanceSettings() {
  return el('div', { class: 'settings-group' },
    choiceSetting('Appearance', [['dark', 'Dark'], ['light', 'Light'], ['auto', 'As the device']], () => THEME, async (key) => {
      const r = await post('/api/ui', { theme: key }).catch(() => null);
      if (r) applyTheme(r.theme);
    }),
    choiceSetting('Buttons', [['text', 'With text'], ['icons', 'Icons only']], () => BUTTONS, async (key) => {
      const r = await post('/api/ui', { buttons: key }).catch(() => null);
      if (r) applyButtons(r.buttons);
    }));
}

// The box has no keyboard but this one. Letters in three rows (so a list fits above it),
// Shift for capitals, and "123" stepping through digits and symbols, more symbols, and
// Czech and German letters. Used by search and by the Wi-Fi password.
const LAYERS = {
  abc: ['qwertyuiop', 'asdfghjkl', 'zxcvbnm'],
  sym: ['1234567890', '!@#$%^&*()', '-_=+.,:;?/'],
  more: ['[]{}<>\\|~`', '\'"€£§°', ''],
  cz: ['ěščřžýáíéú', 'ůťďňóäöüß', '1234567890'],
};
const MORE_LAYERS = ['sym', 'more', 'cz'];
const LAYER_LABEL = { sym: '123', more: '#+=', cz: 'ěšč' };

/**
 * An on-screen keyboard. `field` holds the text ({value}); `onChange` runs after each key;
 * the last key is `doneLabel`, calling `onDone`.
 */
function textKeyboard(field, { onChange, onDone, doneLabel, secret }) {
  if (onPhone()) return phoneEntry(field, { onChange, onDone, doneLabel, secret });
  const kb = el('div', { class: 'keyboard' });
  let layer = 'abc';
  let shift = false;
  const changed = () => onChange && onChange(field.value);
  const type = (ch) => {
    field.value += shift && layer === 'abc' ? ch.toUpperCase() : ch;
    if (shift) { shift = false; draw(); }
    changed();
  };
  const draw = () => {
    const rows = LAYERS[layer];
    const next = layer === 'abc' ? 'sym' : MORE_LAYERS[(MORE_LAYERS.indexOf(layer) + 1) % (MORE_LAYERS.length + 1)] || 'abc';
    kb.replaceChildren(
      ...rows.map((r, i) => {
        const keys = [...r].map((ch) => el('button', { text: shift && layer === 'abc' ? ch.toUpperCase() : ch, onclick: () => type(ch) }));
        const krow = el('div', { class: 'krow' }, keys);
        if (i === rows.length - 1) {
          krow.append(el('button', { class: 'wide', text: '⌫', onclick: () => { field.value = field.value.slice(0, -1); changed(); } }));
        }
        return krow;
      }),
      el('div', { class: 'krow' },
        el('button', { class: shift ? 'wide on' : 'wide', text: '⇧', disabled: layer !== 'abc', onclick: () => { shift = !shift; draw(); } }),
        el('button', { class: 'wide', text: next === 'abc' ? 'abc' : LAYER_LABEL[next], onclick: () => { layer = next; shift = false; draw(); } }),
        el('button', { class: 'wider', text: t('Space'), onclick: () => type(' ') }),
        el('button', { class: 'wide go', text: doneLabel, onclick: () => onDone && onDone(field.value) })));
  };
  draw();
  return kb;
}

/** A phone has a keyboard of its own: a real text field (with its cursor, accents and
 * the browser moving the page above the keyboard) in place of the box's one. */
function onPhone() {
  return document.body.classList.contains('phone');
}

function phoneEntry(field, { onChange, onDone, doneLabel, secret }) {
  const input = el('input', {
    class: 'phone-input',
    type: secret ? 'password' : 'text',
    autocomplete: 'off',
    autocapitalize: 'off',
    autocorrect: 'off',
    spellcheck: 'false',
    enterkeyhint: 'done',
    placeholder: field.placeholder,
  });
  input.value = field.value;
  input.addEventListener('input', () => { field.value = input.value; if (onChange) onChange(field.value); });
  const form = el('form', { class: 'phone-entry', onsubmit: (e) => {
    e.preventDefault();
    input.blur();
    if (onDone) onDone(field.value);
  } }, input, el('button', { class: 'go', type: 'submit', text: doneLabel }));
  // Where the box shows its mirror of the text, at the top: not under the results.
  setTimeout(() => {
    const bar = form.parentNode && form.parentNode.querySelector(':scope > .searchbar');
    if (bar) bar.after(form);
    input.focus();
  }, 0);
  return form;
}

// Search: results come in while typing (the server does not ignore accents, hence the Czech
// layer); "Hide" folds the keyboard away to see all of them.
let searchState = { query: '', results: null, keyboard: true, seq: 0 };

function searchView() {
  return {
    async render(box) {
      const typing = searchState.keyboard || onPhone();
      document.body.classList.toggle('typing', typing && !onPhone());
      const q = el('div', { class: 'q', text: searchState.query, 'data-placeholder': t('Type to search') });
      const area = el('div', { class: 'search-results' });
      const showResults = () => area.replaceChildren(searchState.results ? results(searchState.results) : el('div'));
      const wrap = el('div', { class: 'search-wrap' },
        el('div', { class: 'searchbar', onclick: () => { searchState.keyboard = true; render(); } }, q),
        area,
        typing ? searchKeyboard(q, showResults) : null);
      showResults();
      box.append(wrap);
    },
  };
}

let searchTimer = null;

function scheduleSearch(showResults) {
  clearTimeout(searchTimer);
  const query = searchState.query.trim();
  if (query.length < 2) {
    searchState.results = null;
    showResults();
    return;
  }
  searchTimer = setTimeout(async () => {
    const seq = ++searchState.seq;
    const q = encodeURIComponent(query);
    const [music, bookHits] = await Promise.all([
      get(`/api/library/search?q=${q}`).catch((e) => ({ error: e.message })),
      get(`/api/books/search?q=${q}`).catch(() => []),
    ]);
    const r = Object.assign({}, music, { books: bookHits });
    if (seq !== searchState.seq) return; // a newer search is on its way
    searchState.results = r;
    showResults();
  }, 450);
}

function searchKeyboard(q, showResults) {
  const field = {
    get value() { return searchState.query; },
    set value(v) { searchState.query = v; },
    placeholder: t('Type to search'),
  };
  return textKeyboard(field, {
    onChange: () => { q.textContent = searchState.query; scheduleSearch(showResults); },
    // On a phone, "Done" only puts the phone's keyboard away; the field stays.
    onDone: () => { if (!onPhone()) { searchState.keyboard = false; render(); } },
    doneLabel: onPhone() ? t('Search') : t('Hide'),
  });
}

function results(r) {
  if (r.error) return el('div', { class: 'message bad', text: r.error });
  const out = el('div');
  const books = r.books || [];
  if (!r.artists.length && !r.albums.length && !r.tracks.length && !books.length) {
    return el('div', { class: 'message', text: t('Nothing found. The search matches accents exactly: try ěšč.') });
  }
  if (r.tracks.length) {
    out.append(el('div', { class: 'section-title', text: t('Songs') }),
      el('div', { class: 'rows' }, r.tracks.map((t, i) => trackRow(t, i + 1, () => play({ tracks: r.tracks, start: i }), artistAndAlbum(t)))));
  }
  if (r.albums.length) {
    out.append(el('div', { class: 'section-title', text: t('Albums') }),
      el('div', { class: 'hscroll' }, r.albums.map(albumTile)));
  }
  if (books.length) {
    out.append(el('div', { class: 'section-title', text: t('Audiobooks') }),
      el('div', { class: 'hscroll' }, books.map(bookTile)));
  }
  if (r.artists.length) {
    out.append(el('div', { class: 'section-title', text: t('Artists') }),
      el('div', { class: 'rows' }, r.artists.map((a) => el('div', { class: 'row', onclick: () => push(artistView(a)) },
        a.image ? el('img', { class: 'round', src: a.image, loading: 'lazy', alt: '' }) : null,
        el('div', { class: 'main ellipsis', text: a.name })))));
  }
  return out;
}

// --- audiobooks -------------------------------------------------------------------

const BOOKS = [['home', 'Home'], ['all', 'All books'], ['authors', 'Authors']];

function hours(secs) {
  secs = Math.max(0, Math.round(secs || 0));
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return h ? `${h} h ${m} min` : `${m} min`;
}

/** "3 h 10 min left", "Finished", or the length for a book not started. */
function bookState(b) {
  const p = b.progress;
  if (p && p.completed) return t('Finished');
  if (p && p.position_secs > 0) return t('{t} left', { t: hours(b.duration_secs - p.position_secs) });
  return hours(b.duration_secs);
}

function progressBar(b) {
  const p = b.progress;
  if (!p || p.completed || !b.duration_secs) return null;
  const pct = Math.min(100, (p.position_secs / b.duration_secs) * 100);
  return el('div', { class: 'bar' }, el('div', { style: `width:${pct}%` }));
}

function bookTile(b) {
  return el('div', { class: 'tile', onclick: () => push(bookView(b.id)) },
    cover(b.image),
    progressBar(b),
    el('div', { class: 't ellipsis', text: b.title }),
    el('div', { class: 'a ellipsis muted', text: b.author || bookState(b) }));
}

function booksSection(name, opts = {}) {
  const inner = {
    home: () => booksHome(),
    all: () => allBooksView(opts.sort || 'added', opts.from),
    authors: () => authorsView(opts.from),
  }[name]();
  return {
    async render(box) {
      box.append(el('div', { class: 'subnav' }, BOOKS.map(([key, label]) => el('button', {
        class: key === name ? 'on' : '',
        text: t(label),
        onclick: () => { stack = [booksSection(key)]; render(); },
      }))));
      await inner.render(box);
    },
  };
}

function booksHome() {
  return {
    async render(box) {
      const [going, recent] = await Promise.all([
        get('/api/books/in-progress').catch(() => []),
        get('/api/books?sort=added'),
      ]);
      if (going.length) {
        box.append(shelf(t('Continue listening'), null, el('div', { class: 'hscroll' }, going.map(bookTile))));
      }
      box.append(shelf(t('Recently added'), () => { stack = [booksSection('all', { sort: 'added' })]; render(); },
        el('div', { class: 'hscroll' }, recent.items.slice(0, 24).map(bookTile))));
      if (!going.length && !recent.items.length) box.append(el('div', { class: 'message', text: t('No audiobooks in this library yet.') }));
    },
  };
}

function allBooksView(sort, from) {
  return {
    async render(box) {
      const go = (opts) => { stack = [booksSection('all', opts)]; render(); };
      box.append(el('div', { class: 'toolbar' },
        el('button', { class: sort === 'added' ? 'on' : '', text: t('Recently added'), onclick: () => go({ sort: 'added' }) }),
        el('button', { class: sort === 'title' ? 'on' : '', text: 'A–Z', onclick: () => go({ sort: 'title' }) }),
        el('div', { class: 'spacer' }),
        sort === 'title' ? el('button', { class: 'secondary', text: from ? t('From {l}', { l: from }) : t('Jump to letter'),
          onclick: () => pickLetter(from, (l) => go({ sort: 'title', from: l })) }) : null));
      if (sort === 'title') letterScrub(from, (l) => go({ sort: 'title', from: l }));
      const n = await pagedList(box, `/api/books?sort=${sort}`, sort === 'title' ? from : null, el('div', { class: 'grid' }), (b) => lettered(bookTile(b), b.title));
      if (!n) box.append(el('div', { class: 'message', text: from ? t('Nothing from {l} on.', { l: from }) : 'No audiobooks yet.' }));
    },
  };
}

function authorsView(from) {
  return {
    async render(box) {
      box.append(el('div', { class: 'toolbar' },
        el('div', { class: 'spacer' }),
        el('button', { class: 'secondary', text: from ? t('From {l}', { l: from }) : t('Jump to letter'),
          onclick: () => pickLetter(from, (l) => { stack = [booksSection('authors', { from: l })]; render(); }) })));
      const row = (a) => el('div', { class: 'row', onclick: () => push(authorBooksView(a)) },
        a.image ? el('img', { class: 'round', src: a.image, loading: 'lazy', alt: '' }) : el('div', { class: 'round noimg', style: 'width:44px;height:44px;border-radius:50%' }),
        el('div', { class: 'main ellipsis', text: a.name }));
      letterScrub(from, (l) => { stack = [booksSection('authors', { from: l })]; render(); });
      const n = await pagedList(box, '/api/books/authors', from, el('div', { class: 'rows' }), (a) => lettered(row(a), a.name));
      if (!n) box.append(el('div', { class: 'message', text: from ? t('Nothing from {l} on.', { l: from }) : 'No authors yet.' }));
    },
  };
}

function authorBooksView(author) {
  return {
    async render(box) {
      const list = await get(`/api/books/authors/${encodeURIComponent(author.id)}`);
      box.append(el('h2', { text: author.name, style: 'margin-bottom:14px' }));
      const grid = el('div', { class: 'grid' });
      list.forEach((b) => grid.append(bookTile(b)));
      box.append(grid);
      if (!list.length) box.append(el('div', { class: 'message', text: t('No books.') }));
    },
  };
}

function bookView(id) {
  return {
    async render(box) {
      const d = await get(`/api/books/${encodeURIComponent(id)}`);
      const b = d.book;
      const p = b.progress;
      const started = p && !p.completed && p.position_secs > 0;
      const facts = [d.year, ...d.series.map((s) => `${s.name} ${s.position}`), hours(b.duration_secs)].filter(Boolean).join(' · ');
      box.append(el('div', { class: 'book-head' },
        cover(b.image),
        el('div', { class: 'info' },
          el('div', { class: 'title clamp2', text: b.title }),
          el('div', { class: 'ellipsis', text: b.author }),
          b.narrator ? el('div', { class: 'ellipsis muted', text: t('Narrated by {n}', { n: b.narrator }) }) : null,
          el('div', { class: 'muted', text: facts }),
          progressBar(b),
          el('div', { class: 'buttons' },
            act('play', started ? t('Continue · {t}', { t: bookState(b) }) : t('Play'), 'primary', () => play({ book_id: id })),
            started || (p && p.completed) ? el('button', { class: 'secondary', text: t('Start over'), onclick: () => play({ book_id: id, from_start: true }) }) : null))));
      if (d.description) {
        const text = el('div', { class: 'description clamp', text: d.description });
        text.addEventListener('click', () => text.classList.toggle('clamp'));
        box.append(text);
      }
      if (d.chapters.length) {
        // A chapter starts somewhere in the whole book; the player wants a file and a second in it.
        const starts = [];
        let acc = 0;
        d.parts.forEach((part) => { starts.push(acc); acc += part.duration_secs; });
        const rows = el('div', { class: 'rows' }, d.chapters.map((c, i) => {
          let part = d.parts.findIndex((x) => x.id === c.file_id);
          if (part < 0) part = Math.max(0, starts.findIndex((s, j) => c.start_secs < s + d.parts[j].duration_secs));
          const secs = Math.max(0, c.start_secs - starts[part]);
          return el('div', { class: 'row', onclick: () => play({ book_id: id, part, secs }) },
            el('div', { class: 'n', text: i + 1 }),
            el('div', { class: 'main ellipsis', text: c.title }),
            el('div', { class: 'd', text: clock(c.start_secs) }));
        }));
        box.append(el('div', { class: 'section-title', text: t('Chapters') }), rows);
      }
      if (d.parts.length > 1) {
        const rows = el('div', { class: 'rows' }, d.parts.map((part, i) => el('div', { class: 'row', 'data-id': part.id, onclick: () => play({ book_id: id, part: i, secs: 0 }) },
          el('div', { class: 'n', text: i + 1 }),
          el('div', { class: 'main ellipsis', text: part.title.replace(/^Part (\d+)$/, (_, n) => t('Part {n}', { n })) }),
          el('div', { class: 'd', text: clock(part.duration_secs) }))));
        box.append(el('div', { class: 'section-title', text: t('Parts · {n}', { n: d.parts.length }) }), rows);
      }
    },
  };
}

// --- podcasts ---------------------------------------------------------------------

const PODCASTS = [['home', 'Home'], ['shows', 'Shows']];

function day(iso) {
  if (!iso) return '';
  return new Date(iso).toLocaleDateString(locale(), { day: 'numeric', month: 'short', year: 'numeric' });
}

/** "Played", "12 min left", or the length. */
function episodeState(e) {
  if (e.completed) return t('Played');
  if (e.progress_secs > 0 && e.duration_secs) return t('{t} left', { t: hours(e.duration_secs - e.progress_secs) });
  return e.duration_secs ? hours(e.duration_secs) : '';
}

function episodeRow(e, onTap, showName) {
  const bar = !e.completed && e.progress_secs > 0 && e.duration_secs
    ? el('div', { class: 'bar' }, el('div', { style: `width:${Math.min(100, (e.progress_secs / e.duration_secs) * 100)}%` }))
    : null;
  return el('div', { class: e.completed ? 'row played' : 'row', 'data-id': e.id, onclick: onTap },
    e.image && showName ? el('img', { class: 'square', src: e.image, loading: 'lazy', alt: '' }) : null,
    el('div', { class: 'main' },
      el('div', { class: 'clamp2', text: e.title }),
      // Time left first: a long show name would otherwise push it out of sight.
      el('div', { class: 'sub ellipsis muted', text: (showName ? [episodeState(e), day(e.published_at), e.show_title] : [day(e.published_at), episodeState(e)]).filter(Boolean).join(' · ') }),
      bar));
}

function showTile(sh) {
  return el('div', { class: 'tile', onclick: () => push(showView(sh.id)) },
    cover(sh.image),
    el('div', { class: 't clamp2', text: sh.title }),
    el('div', { class: 'a ellipsis muted', text: sh.unplayed ? tn(sh.unplayed, 'unplayed') : (sh.host || '') }));
}

function podcastsSection(name) {
  const inner = { home: () => podcastsHome(), shows: () => showsView() }[name]();
  return {
    async render(box) {
      box.append(el('div', { class: 'subnav' }, PODCASTS.map(([key, label]) => el('button', {
        class: key === name ? 'on' : '',
        text: t(label),
        onclick: () => { stack = [podcastsSection(key)]; render(); },
      }))));
      await inner.render(box);
    },
  };
}

function podcastsHome() {
  return {
    async render(box) {
      const [going, latest, shows] = await Promise.all([
        get('/api/podcasts/in-progress').catch(() => []),
        get('/api/podcasts/latest').catch(() => []),
        get('/api/podcasts'),
      ]);
      if (going.length) {
        box.append(shelf(t('Continue listening'), null,
          el('div', { class: 'rows' }, going.map((e, i) => episodeRow(e, () => push(episodeView(going, i)), true)))));
      }
      if (shows.length) {
        box.append(shelf(t('Shows'), () => { stack = [podcastsSection('shows')]; render(); },
          el('div', { class: 'hscroll' }, shows.map(showTile))));
      }
      if (latest.length) {
        box.append(shelf(t('New episodes'), null,
          el('div', { class: 'rows' }, latest.slice(0, 10).map((e, i) => episodeRow(e, () => push(episodeView(latest, i)), true)))));
      }
      if (!shows.length) box.append(el('div', { class: 'message', text: t('No podcasts yet. Follow shows in the own.audio app.') }));
    },
  };
}

function showsView() {
  return {
    async render(box) {
      const shows = await get('/api/podcasts');
      const grid = el('div', { class: 'grid' }, shows.map(showTile));
      box.append(grid);
      if (!shows.length) box.append(el('div', { class: 'message', text: t('No podcasts yet. Follow shows in the own.audio app.') }));
    },
  };
}

function showView(id) {
  return {
    async render(box) {
      const first = await get(`/api/podcasts/${encodeURIComponent(id)}`);
      const sh = first.show;
      const episodes = first.episodes;
      box.append(el('div', { class: 'book-head' },
        cover(sh.image),
        el('div', { class: 'info' },
          el('div', { class: 'title clamp2', text: sh.title }),
          sh.host ? el('div', { class: 'ellipsis', text: sh.host }) : null,
          el('div', { class: 'muted', text: sh.unplayed ? tn(sh.unplayed, 'unplayed') : t('All played') }))));
      if (sh.description) {
        const text = el('div', { class: 'description clamp', text: sh.description });
        text.addEventListener('click', () => text.classList.toggle('clamp'));
        box.append(text);
      }
      const rows = el('div', { class: 'rows' });
      // The list grows as it is read; a tap plays from the list as it stands then.
      const add = (list) => list.forEach((e) => {
        const i = episodes.indexOf(e);
        rows.append(episodeRow(e, () => push(episodeView(episodes, i)), false));
      });
      add(episodes);
      box.append(el('div', { class: 'section-title', text: t('Episodes') }), rows);
      if (episodes.length >= 50) {
        pager(box, async () => {
          const more = await get(`/api/podcasts/${encodeURIComponent(id)}?offset=${episodes.length}`);
          episodes.push(...more.episodes);
          add(more.episodes);
          return more.episodes.length >= 50;
        });
      }
    },
  };
}

function episodeView(list, index) {
  return {
    async render(box) {
      const e = list[index];
      const started = !e.completed && e.progress_secs > 0;
      const markBtn = el('button', { class: 'secondary', text: e.completed ? t('Mark as unplayed') : t('Mark as played') });
      markBtn.addEventListener('click', async () => {
        const completed = !e.completed;
        await post('/api/podcasts/mark', { ids: [e.id], completed }).catch(() => null);
        e.completed = completed;
        if (completed) e.progress_secs = 0;
        render();
      });
      box.append(el('div', { class: 'book-head' },
        cover(e.image),
        el('div', { class: 'info' },
          el('div', { class: 'title clamp2', text: e.title }),
          el('div', { class: 'ellipsis', text: e.show_title }),
          el('div', { class: 'muted', text: [day(e.published_at), episodeState(e)].filter(Boolean).join(' · ') }),
          el('div', { class: 'buttons' },
            act('play', started ? t('Continue · {t}', { t: episodeState(e) }) : t('Play'), 'primary', () => play({ episodes: list, start: index })),
            markBtn))));
      if (e.description) box.append(el('div', { class: 'description', text: e.description }));
    },
  };
}

// --- language ---------------------------------------------------------------------

function languageSettings() {
  // Each language is named in itself, so either can be found whichever is on.
  const choice = (code, label) => el('button', {
    class: LANG === code ? 'on' : 'secondary',
    text: label,
    onclick: async () => {
      const r = await post('/api/ui', { language: code }).catch(() => null);
      if (!r) return;
      LANG = r.language;
      translatePage();
      labelPlayer();
      render();
    },
  });
  return el('div', { class: 'settings-group' },
    el('div', { class: 'setting' }, el('span', { text: t('Language') }),
      el('div', { class: 'buttons' }, choice('cs', 'Čeština'), choice('en', 'English'))));
}

// --- network ----------------------------------------------------------------------

async function networkSettings() {
  const w = await get('/api/wifi').catch(() => null);
  const group = el('div', { class: 'settings-group' }, el('div', { class: 'section-title', text: t('Network') }));
  if (!w) return group.append(el('p', { class: 'muted', text: t('Network settings are not available.') })), group;
  const wifiText = !w.radio ? t('Off') : w.connected ? `${w.connected}${w.address ? ` · ${w.address}` : ''}` : t('Not connected');
  group.append(
    el('div', { class: 'setting' }, el('span', { text: t('Ethernet') }), el('span', { text: w.ethernet || t('Not connected') })),
    el('div', { class: 'setting' }, el('span', { text: t('Wi-Fi') }), el('span', { text: wifiText }),
      el('button', { class: 'secondary', text: t('Wi-Fi networks'), onclick: () => push(wifiView()) })));
  return group;
}

function signalText(n) {
  return [n.in_use ? t('Connected') : n.saved ? t('Saved') : null, n.secure ? t('Password') : t('Open'), `${n.signal} %`].filter(Boolean).join(' · ');
}

function wifiView() {
  return {
    async render(box) {
      const status = await get('/api/wifi');
      const msg = el('p', { class: 'muted', style: 'min-height:1.4em' });
      const toggle = el('button', {
        class: status.radio ? 'on' : 'secondary',
        text: status.radio ? t('Wi-Fi is on') : t('Wi-Fi is off'),
        onclick: async () => {
          await post('/api/wifi/radio', { on: !status.radio }).catch((e) => { msg.textContent = e.message; });
          render();
        },
      });
      const list = el('div', { class: 'rows' });
      const rescan = el('button', { class: 'secondary', text: t('Search again'), onclick: () => scan() });
      box.append(el('div', { class: 'toolbar' }, el('h2', { text: t('Wi-Fi') }), el('div', { class: 'spacer' }), toggle, status.radio ? rescan : null), msg);
      if (!status.radio) return;
      box.append(list);

      const connect = async (n) => {
        if (n.in_use) return;
        if (n.secure && !n.saved) return push(passwordView(n.ssid));
        msg.className = 'muted';
        msg.textContent = t('Connecting to {s}…', { s: n.ssid });
        try {
          await post('/api/wifi/connect', { ssid: n.ssid });
          msg.textContent = t('Connected to {s}.', { s: n.ssid });
          scan();
        } catch (e) {
          msg.className = 'bad';
          msg.textContent = t(e.message);
        }
      };
      const forget = async (n) => {
        await post('/api/wifi/forget', { ssid: n.ssid }).catch((e) => { msg.textContent = e.message; });
        scan();
      };
      async function scan() {
        list.replaceChildren(el('div', { class: 'message', text: t('Looking for networks…') }));
        let nets;
        try {
          nets = await get('/api/wifi/scan');
        } catch (e) {
          list.replaceChildren(el('div', { class: 'message bad', text: e.message }));
          return;
        }
        list.replaceChildren(...nets.map((n) => el('div', { class: n.in_use ? 'row current' : 'row', onclick: () => connect(n) },
          el('div', { class: 'main' },
            el('div', { class: 'ellipsis', text: n.ssid }),
            el('div', { class: 'sub muted', text: signalText(n) })),
          n.saved ? el('button', { class: 'secondary', text: t('Forget'), onclick: (e) => { e.stopPropagation(); forget(n); } }) : null)));
        if (!nets.length) list.replaceChildren(el('div', { class: 'message', text: t('No networks in reach.') }));
      }
      scan();
    },
  };
}

function passwordView(ssid) {
  return {
    async render(box) {
      let shown = false;
      const field = { value: '' };
      const text = el('div', { class: 'q' });
      const draw = () => { text.textContent = shown ? field.value : '•'.repeat(field.value.length); };
      const msg = el('p', { class: 'muted', style: 'min-height:1.4em' });
      const connect = async () => {
        if (!field.value) return;
        msg.className = 'muted';
        msg.textContent = t('Connecting to {s}…', { s: ssid });
        try {
          await post('/api/wifi/connect', { ssid, password: field.value });
          field.value = '';
          back();
        } catch (e) {
          msg.className = 'bad';
          msg.textContent = t(e.message);
        }
      };
      draw();
      document.body.classList.add('typing');
      box.append(el('div', { class: 'search-wrap' },
        el('div', { class: 'toolbar' }, el('h2', { class: 'ellipsis', text: t('Password for {s}', { s: ssid }) })),
        el('div', { class: 'searchbar' }, text,
          el('button', { class: 'secondary', text: t('Show'), onclick: (e) => { shown = !shown; e.currentTarget.textContent = shown ? t('Hide') : t('Show'); draw(); } })),
        msg,
        el('div', { style: 'flex:1' }),
        textKeyboard(field, { onChange: draw, onDone: connect, doneLabel: t('Connect'), secret: true })));
    },
  };
}

// --- first-run wizard -------------------------------------------------------------
// Language → network → server → sign-in → sound → name → done. The steps are views on the
// same stack as the rest of the screen, so Back works and the Wi-Fi page is reused; the
// tabs and the now-playing bar step aside while it runs.

let setup = { done: true };

function startWizard() {
  setup.done = false;
  document.body.classList.add('wizard');
  $('signin').hidden = true;
  $('nowplaying').hidden = true;
  $('app').hidden = false;
  currentTab = null;
  // Back where it was after a reload: signed in already means sound next; a language
  // chosen (here or on the setup Wi-Fi) means the network next.
  stack = setup.signed_in ? [wizSound()] : setup.language_set ? [wizNetwork()] : [wizLanguage()];
  render();
}

function finishWizard() {
  setup.done = true;
  document.body.classList.remove('wizard');
  signedIn = null; // so the session's next answer lands on Home
  refreshSession();
}

/** One step: a title, a line of explanation, its content, and the way on. */
function wizStep(box, { step, title, intro, content, next, greeting }) {
  // The DOM's own append writes null out as text; leave the missing parts out instead.
  box.append(el('div', { class: greeting ? 'wiz hero' : 'wiz' }, ...[
    wizProgress(step),
    greeting ? brand(88, { animated: true, big: true }) : null,
    el('h2', { text: title }),
    intro ? el('p', { class: 'wiz-intro', text: intro }) : null,
    content,
    next ? el('div', { class: 'wiz-next' }, next) : null,
  ].filter(Boolean)));
}

/** Seven bars, the steps done and this one lit, with the count for a screen reader. */
function wizProgress(step) {
  return el('div', {
    class: 'ob-progress wiz-progress', role: 'progressbar', 'aria-valuemin': 1, 'aria-valuemax': 7, 'aria-valuenow': step,
    'aria-label': t('Step {n} of {m}', { n: step, m: 7 }),
  }, ...Array.from({ length: 7 }, (_, i) => el('span', { class: i < step ? 'on' : '' })));
}

/** Copies `text`. The box is served over plain http, where browsers do not offer the
 * clipboard API: a selected, off-screen text field and the old copy command still work
 * there (Safari on iPhone too). False when neither did; the caller then shows the text
 * selected, for a long press. */
async function copyText(text) {
  try {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch { /* the old way below */ }
  const area = el('textarea', { readonly: true, style: 'position:fixed;top:0;left:-9999px;opacity:0' });
  area.value = text;
  document.body.append(area);
  area.focus();
  area.select();
  area.setSelectionRange(0, text.length);
  let done = false;
  try { done = document.execCommand('copy'); } catch { done = false; }
  area.remove();
  return done;
}

/** Selects the text of `node`, so a long press copies it when the button could not. */
function selectText(node) {
  const range = document.createRange();
  range.selectNodeContents(node);
  const sel = window.getSelection();
  sel.removeAllRanges();
  sel.addRange(range);
}

/** A choice as a tile: an icon, a name and a line under it. */
function wizChoice(icon, title, sub, onclick) {
  return el('button', { class: 'choice', onclick }, obIcon(icon, 30),
    el('span', { class: 'choice-text' }, el('strong', { text: title }), sub ? el('span', { class: 'muted', text: sub }) : null));
}

function wizLanguage() {
  return {
    async render(box) {
      const choose = async (code) => {
        const r = await post('/api/ui', { language: code }).catch(() => null);
        LANG = r ? r.language : code;
        translatePage();
        labelPlayer();
        push(wizNetwork());
      };
      wizStep(box, {
        step: 1,
        greeting: true,
        title: 'Jazyk · Language',
        content: el('div', { class: 'wiz-choices' },
          wizChoice('globe', 'Čeština', 'Česky', () => choose('cs')),
          wizChoice('globe', 'English', 'In English', () => choose('en'))),
      });
    },
  };
}

function wizNetwork() {
  return {
    async render(box) {
      const w = await get('/api/wifi').catch(() => null);
      const online = w && (w.ethernet || w.connected);
      const status = !w ? t('Network settings are not available.')
        : w.ethernet ? t('Connected by cable ({a}).', { a: w.ethernet })
          : w.connected ? t('Connected to Wi-Fi {s} ({a}).', { s: w.connected, a: w.address || '' })
            : t('Not connected yet.');
      wizStep(box, {
        step: 2,
        title: t('Network'),
        intro: t('The box needs the network to reach your library.'),
        content: el('div', { class: 'settings-group' },
          el('div', { class: online ? 'wiz-status ok' : 'wiz-status warn' }, obIcon(online ? 'check' : 'wifi', 26), el('span', { text: status })),
          w && !w.ethernet ? el('div', {}, el('button', { class: 'secondary', text: t('Wi-Fi networks'), onclick: () => push(wifiView()) })) : null),
        next: el('button', { class: 'primary', text: online ? t('Continue') : t('Continue anyway'), onclick: () => push(wizServer()) }),
      });
    },
  };
}

/** The server step's own fetch: a failed check comes back as 400 with the reason as data. */
async function useServer(url) {
  const r = await fetch('/api/setup/server', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ url }),
  }).catch(() => null);
  if (!r) return { result: 'unreachable', url };
  const data = await r.json().catch(() => ({}));
  return data.result ? data : { result: 'bad_address', url };
}

function serverProblem(c) {
  switch (c.result) {
    case 'unreachable': return t('Nothing answered at {u}. Check the address, and that the server is on.', { u: c.url });
    case 'not_own_audio': return t('{u} answered, but it is not an own.audio server.', { u: c.url });
    case 'no_device_sign_in': return t('This own.audio server ({v}) is too old to sign in with a code. Update it first.', { v: c.version || '?' });
    default: return t('That is not an address.');
  }
}

function wizServer() {
  return {
    async render(box) {
      const st = await get('/api/setup');
      const msg = el('p', { class: 'muted', style: 'min-height:1.4em' });
      const hosted = async () => {
        msg.className = 'muted';
        msg.textContent = t('Checking {u}…', { u: st.hosted_url });
        const c = await useServer(st.hosted_url);
        if (c.result === 'ok') push(wizSignIn());
        else { msg.className = 'bad'; msg.textContent = serverProblem(c); }
      };
      wizStep(box, {
        step: 3,
        title: t('Your library'),
        intro: t('Where your music, audiobooks and podcasts are.'),
        content: el('div', {},
          el('div', { class: 'wiz-choices' },
            wizChoice('cloud', 'own.audio', t('The hosted service at own.audio'), hosted),
            wizChoice('server', t('Your own server at home'), t('own.audio running on your computer or NAS'), () => push(wizServerAddress(st)))),
          msg),
      });
    },
  };
}

function wizServerAddress(st) {
  return {
    async render(box) {
      const start = st.server_url && st.server_url !== st.hosted_url ? st.server_url : 'http://';
      const field = { value: start };
      const text = el('div', { class: 'q', text: field.value });
      const msg = el('p', { class: 'muted', style: 'min-height:1.4em' });
      const check = async () => {
        msg.className = 'muted';
        msg.textContent = t('Checking {u}…', { u: field.value });
        const c = await useServer(field.value);
        if (c.result === 'ok') {
          document.body.classList.remove('typing');
          push(wizSignIn());
        } else {
          msg.className = 'bad';
          msg.textContent = serverProblem(c);
        }
      };
      document.body.classList.add('typing');
      box.append(el('div', { class: 'search-wrap' },
        wizProgress(3),
        el('h2', { text: t('Server address') }),
        el('p', { class: 'muted small', text: t('As in the own.audio app, for example http://192.168.1.10:8099') }),
        el('div', { class: 'searchbar' }, text),
        msg,
        el('div', { style: 'flex:1' }),
        textKeyboard(field, { onChange: () => { text.textContent = field.value; }, onDone: check, doneLabel: t('Check') })));
    },
  };
}

let wizPoll = null;

function wizSignIn() {
  return {
    async render(box) {
      clearInterval(wizPoll);
      const st = await get('/api/setup');
      const next = el('button', { class: 'primary', text: t('Continue'), onclick: () => { clearInterval(wizPoll); push(wizSound()); } });
      if (st.signed_in) {
        const s = await get('/api/session');
        wizStep(box, { step: 4, title: t('Signing in'), content: el('p', { class: 'ok', text: t('Signed in as {u}.', { u: s.user.display_name || s.user.email }) }), next });
        return;
      }
      const qr = el('img', { class: 'wiz-qr', alt: '', width: 220, height: 220 });
      const code = el('p', { class: 'code' });
      const where = el('p', { class: 'link' });
      const msg = el('p', { class: 'muted' });
      // On a phone the page opens in another tab; this one notices the approval by itself.
      const open = el('a', { class: 'button primary wiz-open', target: '_blank', rel: 'noopener', hidden: true, text: t('Confirm in own.audio') });
      const copy = el('button', { class: 'secondary', hidden: true, text: t('Copy the code'), onclick: async () => {
        if (await copyText(code.textContent)) copy.textContent = t('Copied');
        else { selectText(code); copy.textContent = t('Hold the code to copy it'); }
      } });
      const waiting = el('p', { class: 'wiz-waiting muted', hidden: true }, el('span', { class: 'wiz-dot' }), t('Waiting for the approval; this page moves on by itself.'));
      wizStep(box, {
        step: 4,
        title: t('Signing in'),
        intro: HEADLESS
          ? t('Confirm the code in your own.audio account. No password is typed here.')
          : t('Scan the code with your phone, or open the address below and enter the code.'),
        content: el('div', { class: 'wiz-signin' }, qr, el('div', { class: 'wiz-code' }, where, code, msg, copy), open, waiting),
      });
      await post('/api/sign-in').catch((e) => { msg.textContent = t(e.message); });
      let shown = null;
      const view = stack[stack.length - 1];
      // Also at once when the tab is back: a phone that confirmed the code in another tab or
      // app may have put this one to sleep, and its timers with it.
      const tick = async () => {
        if (stack[stack.length - 1] !== view) { clearInterval(wizPoll); window.removeEventListener('ownbox-wake', tick); return; } // left the step
        const s = await get('/api/session').catch(() => null);
        if (!s) return;
        if (s.state === 'signed_in') {
          clearInterval(wizPoll);
          window.removeEventListener('ownbox-wake', tick);
          push(wizSound());
        } else if (s.state === 'pending') {
          if (shown !== s.user_code) {
            shown = s.user_code;
            qr.src = `/api/sign-in/qr.svg?c=${encodeURIComponent(s.user_code)}`;
          }
          qr.hidden = !s.link || HEADLESS;
          showLink(where, s.link);
          if (HEADLESS && s.link) { open.href = s.link; open.hidden = false; copy.hidden = false; waiting.hidden = false; }
          code.textContent = s.user_code;
          msg.textContent = t('Valid for {t}', { t: clock(s.expires_in) });
        } else {
          msg.textContent = s.error ? t(s.error) : '';
        }
      };
      window.addEventListener('ownbox-wake', tick);
      wizPoll = setInterval(tick, 1000);
    },
  };
}

function wizSound() {
  return {
    async render(box) {
      const n = await get('/api/player').catch(() => ({ volume: 100 }));
      const slider = el('input', { type: 'range', min: 0, max: 100, value: Math.round(n.volume), class: 'slider' });
      let timer = null;
      slider.addEventListener('input', () => {
        clearTimeout(timer);
        timer = setTimeout(() => post('/api/player/volume', { volume: Number(slider.value) }).catch(() => null), 100);
      });
      const msg = el('p', { class: 'muted', style: 'min-height:1.4em' });
      wizStep(box, {
        step: 5,
        title: t('Sound'),
        intro: t("Connect the box's audio output to the speaker's AUX input, then play the test tone."),
        content: el('div', { class: 'settings-group' },
          el('div', { class: 'wiz-choices' },
            wizChoice('speaker', t('Play test tone'), t('A short tone from the box'), async () => {
              msg.textContent = t('Playing…');
              await post('/api/test-tone').then(() => { msg.textContent = t('Test tone sent to the speaker.'); })
                .catch((e) => { msg.textContent = t('Failed: {e}', { e: t(e.message) }); });
            })),
          el('div', { class: 'setting' }, el('span', { text: t('Volume') }), slider),
          msg),
        next: el('button', { class: 'primary', text: t('Continue'), onclick: () => push(wizName()) }),
      });
    },
  };
}

/** The box's name: a wizard step, or (`fromSettings`) the same page opened from Settings,
 * which goes back when saved. */
function wizName(fromSettings = false) {
  return {
    async render(box) {
      const st = await get('/api/setup');
      const field = { value: st.box_name || 'own.audio box' };
      const text = el('div', { class: 'q', text: field.value });
      const msg = el('p', { class: 'bad', style: 'min-height:1.4em' });
      const save = async () => {
        try {
          await post('/api/setup/name', { name: field.value });
          document.body.classList.remove('typing');
          if (fromSettings) back();
          else push(wizDone());
        } catch (e) {
          msg.textContent = t(e.message);
        }
      };
      if (onPhone()) {
        const address = el('span', { class: 'ob-addr', text: `${obHostName(field.value)}.local` });
        const input = el('input', { type: 'text', class: 'ob-input big', value: field.value, maxlength: 60, autocomplete: 'off',
          oninput: (e) => { field.value = e.target.value; address.textContent = `${obHostName(field.value)}.local`; } });
        const rooms = LANG === 'cs' ? ['Obývák', 'Kuchyň', 'Ložnice', 'Dětský pokoj', 'Pracovna'] : ['Living room', 'Kitchen', 'Bedroom', 'Kids\' room', 'Study'];
        const content = el('div', { class: 'wiz-name' }, input,
          el('div', { class: 'ob-chips' }, ...rooms.map((r) => el('button', { class: 'secondary', text: r,
            onclick: () => { input.value = r; input.dispatchEvent(new Event('input')); } }))),
          el('p', { class: 'ob-note' }, t('Address: '), address), msg);
        if (fromSettings) {
          box.append(el('div', { class: 'wiz' }, el('h2', { text: t('Name the box') }),
            el('p', { class: 'wiz-intro', text: t('How it appears in AirPlay and Music Assistant, and among your devices from the next sign-in.') }),
            content, el('div', { class: 'wiz-next' }, el('button', { class: 'primary', text: t('Save'), onclick: save }))));
        } else {
          wizStep(box, { step: 6, title: t('What should we call it?'), intro: t("The name shows in AirPlay and in your own.audio apps, and it becomes the box's address."),
            content, next: el('button', { class: 'primary', text: t('Continue'), onclick: save }) });
        }
        return;
      }
      document.body.classList.add('typing');
      box.append(el('div', { class: 'search-wrap' },
        fromSettings ? null : wizProgress(6),
        el('h2', { text: t('Name the box') }),
        el('p', { class: 'muted small', text: fromSettings
          ? t('How it appears in AirPlay and Music Assistant, and among your devices from the next sign-in.')
          : t('How it appears among your devices, for example "Kitchen".') }),
        el('div', { class: 'searchbar' }, text),
        msg,
        el('div', { style: 'flex:1' }),
        textKeyboard(field, { onChange: () => { text.textContent = field.value; }, onDone: save, doneLabel: fromSettings ? t('Save') : t('Continue') })));
    },
  };
}

function nameSettings() {
  const name = el('span', { class: 'ellipsis', text: '…' });
  get('/api/setup').then((st) => { name.textContent = st.box_name || 'own.audio box'; }).catch(() => null);
  return el('div', { class: 'settings-group' },
    el('div', { class: 'setting' }, el('span', { text: t('Box name') }), name,
      el('button', { class: 'secondary', text: t('Change'), onclick: () => push(wizName(true)) })));
}

function wizDone() {
  return {
    async render(box) {
      wizStep(box, {
        step: 7,
        greeting: true,
        title: t('All set'),
        intro: t('Your library is on the box. Settings can change any of this later.'),
        next: el('button', { class: 'primary', text: t('Start listening'), onclick: async () => {
          await post('/api/setup/done', { done: true }).catch(() => null);
          finishWizard();
        } }),
      });
    },
  };
}

function serverSettings() {
  const group = el('div', { class: 'settings-group' }, el('div', { class: 'section-title', text: t('Library') }));
  const url = el('span', { class: 'ellipsis', text: '…' });
  get('/api/setup').then((st) => { url.textContent = st.server_url; }).catch(() => null);
  group.append(
    el('div', { class: 'setting' }, el('span', { text: t('Server') }), url),
    el('div', { class: 'buttons' },
      el('button', { class: 'secondary', text: t('Run the setup again'), onclick: async () => {
        await post('/api/setup/done', { done: false }).catch(() => null);
        startWizard();
      } }),
      resetButton()));
  return group;
}

/** Sign out and start over: two taps, the second within five seconds, since it forgets the
 * sign-in, the queue and every paired phone. The Wi-Fi, sound card and name stay. */
function resetButton() {
  let armed = null;
  const button = el('button', { class: 'secondary danger', text: t('Sign out and start over'), onclick: async () => {
    if (!armed) {
      button.textContent = t('Sure? Tap again');
      button.classList.add('armed');
      armed = setTimeout(() => { armed = null; button.textContent = t('Sign out and start over'); button.classList.remove('armed'); }, 5000);
      return;
    }
    clearTimeout(armed);
    button.disabled = true;
    await post('/api/setup/reset').catch(() => null);
    // The page starts over too: the wizard on the screen; on a phone, pairing first, as
    // the box has forgotten every phone.
    location.replace('/');
  } });
  return button;
}

// --- equalizer --------------------------------------------------------------------
// The apps' equalizer: six bands at fixed frequencies, ±12 dB, a preamp, seven presets.
// Each change plays at once; the box saves it here and on the server (as the apps do).

function freqLabel(hz) {
  return hz >= 1000 ? `${hz % 1000 ? (hz / 1000).toFixed(1) : hz / 1000}K` : String(hz);
}

function eqView() {
  return {
    async render(box) {
      const d = await get('/api/eq');
      const cur = Object.assign({}, d.eq, { bands_db: d.eq.bands_db.slice() });
      let timer = null;
      const save = () => {
        clearTimeout(timer);
        timer = setTimeout(() => post('/api/eq', cur).catch(() => null), 120);
      };
      const dB = (v) => `${v > 0 ? '+' : ''}${Number(v).toFixed(1)} dB`;

      const toggle = el('button', {});
      const drawToggle = () => {
        toggle.className = cur.enabled ? 'on' : 'secondary';
        toggle.textContent = cur.enabled ? t('On') : t('Off');
      };
      toggle.addEventListener('click', () => { cur.enabled = !cur.enabled; drawToggle(); save(); });
      drawToggle();

      const sliders = [];
      const bands = el('div', { class: 'eq-bands' }, d.frequencies_hz.map((hz, i) => {
        const value = el('div', { class: 'eq-value', text: dB(cur.bands_db[i]) });
        const input = el('input', { type: 'range', class: 'vslider', min: -d.limit_db, max: d.limit_db, step: 0.5, value: cur.bands_db[i] });
        input.addEventListener('input', () => {
          cur.bands_db[i] = Number(input.value);
          cur.preset_name = null; // a curve of one's own
          cur.enabled = true;
          value.textContent = dB(cur.bands_db[i]);
          drawToggle();
          drawPresets();
          save();
        });
        sliders.push({ input, value });
        return el('div', { class: 'eq-band' }, value, input, el('div', { class: 'eq-freq', text: freqLabel(hz) }));
      }));

      const preValue = el('span', { class: 'muted', text: dB(cur.preamp_db) });
      const pre = el('input', { type: 'range', class: 'slider', min: -d.limit_db, max: d.limit_db, step: 0.5, value: cur.preamp_db });
      pre.addEventListener('input', () => { cur.preamp_db = Number(pre.value); preValue.textContent = dB(cur.preamp_db); save(); });

      const presets = el('div', { class: 'buttons eq-presets' });
      const drawPresets = () => presets.replaceChildren(...d.presets.map((p) => el('button', {
        class: cur.preset_name === p.name ? 'on' : 'secondary',
        text: t(p.name),
        onclick: () => {
          cur.bands_db = p.bands_db.slice();
          cur.preset_name = p.name;
          cur.enabled = true;
          sliders.forEach((sl, i) => { sl.input.value = cur.bands_db[i]; sl.value.textContent = dB(cur.bands_db[i]); });
          drawToggle();
          drawPresets();
          save();
        },
      })));
      drawPresets();

      box.append(
        el('div', { class: 'toolbar' }, el('h2', { text: t('Equalizer') }), el('div', { class: 'spacer' }), toggle),
        bands,
        el('div', { class: 'setting' }, el('span', { text: t('Preamp') }), pre, preValue),
        el('div', { class: 'section-title', text: t('Presets') }),
        presets);
    },
  };
}

// --- updates -------------------------------------------------------------------------

async function updateSettings() {
  const group = el('div', { class: 'settings-group' }, el('div', { class: 'section-title', text: t('Updates') }));
  const u = await get('/api/update').catch(() => null);
  if (!u) return group;
  const msg = el('p', { class: 'muted small' });
  const redraw = async () => group.replaceWith(await updateSettings());
  const act = (label, path, primary) => el('button', {
    class: primary ? '' : 'secondary',
    text: label,
    onclick: async (ev) => {
      ev.currentTarget.disabled = true;
      msg.textContent = t('Working…');
      try {
        await post(path);
        if (path !== '/api/update/check') {
          // The box restarts with the other version: the page waits until the box answers
          // with a version other than this one (or with a result), then reloads.
          msg.textContent = t('Installing. The box restarts in a moment.');
          const until = Date.now() + 180000;
          const wait = async () => {
            const now = await get('/api/update').catch(() => null);
            if ((now && (now.current !== u.current || (now.last_result && now.last_result !== u.last_result && !now.state))) || Date.now() > until) {
              location.reload();
              return;
            }
            setTimeout(wait, 2000);
          };
          setTimeout(wait, 3000);
          return;
        }
        redraw();
      } catch (e) {
        msg.textContent = t('Failed: {e}', { e: t(e.message) });
        ev.currentTarget.disabled = false;
      }
    },
  });
  const buttons = el('div', { class: 'buttons' });
  group.append(el('div', { class: 'setting' }, el('span', { text: t('Version') }), el('span', { text: `ownbox ${u.current}` })));
  if (!u.enabled) {
    group.append(el('p', { class: 'muted small', text: t('This box does not update itself; it is updated from the computer it was set up from.') }));
  } else if (u.available) {
    group.append(el('p', { text: t('Version {v} is available.', { v: u.available.version }) }));
    if (u.available.notes) group.append(el('pre', { class: 'notes', text: u.available.notes.trim().slice(0, 1500) }));
    buttons.append(act(t('Install'), '/api/update/install', true));
  } else {
    group.append(el('p', { class: 'muted small', text: t('Up to date.') }));
    buttons.append(act(t('Check now'), '/api/update/check'));
  }
  if (u.previous) buttons.append(act(t('Go back to {v}', { v: u.previous }), '/api/update/rollback'));
  if (u.enabled) {
    const auto = (on) => el('button', {
      class: u.auto === on ? 'on' : 'secondary',
      text: on ? t('At night by itself') : t('Only when I tap Install'),
      onclick: async () => { await post('/api/update/auto', { on }).catch(() => null); redraw(); },
    });
    group.append(
      el('div', { class: 'setting' }, el('span', { text: t('Install updates') })),
      el('div', { class: 'buttons' }, auto(true), auto(false)),
      el('p', { class: 'muted small', text: t('Between 3 and 5 at night, when nothing is playing; Debian\'s security updates once a week as well. The box may restart.') }),
    );
  }
  if (buttons.childElementCount) group.append(buttons);
  if (u.last_result) group.append(el('p', { class: 'muted small', text: t('Last: {r}', { r: u.last_result }) }));
  if (u.system_result) group.append(el('p', { class: 'muted small', text: t('System: {r}', { r: u.system_result }) }));
  group.append(msg);
  return group;
}

// --- sound output -------------------------------------------------------------------

async function outputSettings() {
  const a = await get('/api/audio').catch(() => null);
  if (!a || a.outputs.length < 2) return el('div');
  const group = el('div', { class: 'settings-group' }, el('div', { class: 'section-title', text: t('Sound output') }));
  const label = (o) => (o.kind === 'analog' ? t('3.5 mm jack') : o.label);
  group.append(el('div', { class: 'buttons wrap' }, ...a.outputs.map((o) => el('button', {
    class: o.device === a.chosen ? 'on' : 'secondary',
    text: label(o),
    onclick: async () => {
      if (o.device === a.chosen) return;
      const r = await post('/api/audio/output', { device: o.device }).catch(() => null);
      if (r) group.replaceWith(await outputSettings());
    },
  }))));
  group.append(el('p', { class: 'muted small', text: t('Playback pauses when the output changes.') }));
  return group;
}

/** A sound card on the GPIO header (a HAT): one without an EEPROM is named here. */
async function hatSettings() {
  const h = await get('/api/audio/hats').catch(() => null);
  if (!h) return el('div');
  const group = el('div', { class: 'settings-group' }, el('div', { class: 'section-title', text: t('Sound card on the pins') }));
  const select = el('select', {}, ...h.choices.map((c) => el('option', { value: c.id, text: t(c.label) })));
  if (h.current != null) select.value = h.current;
  else select.prepend(el('option', { value: '', text: t('Set in config.toml'), selected: true }));
  select.addEventListener('change', async () => {
    if (select.value === '') return;
    const r = await post('/api/audio/hat', { id: Number(select.value) }).catch(() => null);
    if (r) toast(t('Saved. The box restarts if the card changed.'));
  });
  group.append(select, el('p', { class: 'muted small', text: t('Only for a card on the pins that the box does not find itself. A new card restarts the box.') }));
  return group;
}

// --- outside sources -------------------------------------------------------------------

async function sourcesSettings() {
  const group = el('div', { class: 'settings-group' }, el('div', { class: 'section-title', text: t('Play to the box') }));
  const st = await get('/api/outside/sources').catch(() => null);
  if (!st) return group;
  const row = (key, label, help) => {
    const src = st[key];
    if (!src.installed) return null;
    const btn = (on) => el('button', {
      class: src.enabled === on ? 'on' : 'secondary',
      text: t(on ? 'On' : 'Off'),
      onclick: async () => {
        const r = await post('/api/outside/sources', { [key]: on }).catch(() => null);
        if (r) group.replaceWith(await sourcesSettings());
      },
    });
    return el('div', {},
      el('div', { class: 'setting' }, el('span', { text: label }), el('div', { class: 'buttons' }, btn(true), btn(false))),
      el('p', { class: 'muted small', text: help }));
  };
  const rows = [
    row('airplay', 'AirPlay', t('iPhone, iPad and Mac can play to the box from any app.')),
    row('music_assistant', 'Music Assistant', t('Music Assistant finds the box as a Squeezelite player.')),
  ].filter(Boolean);
  if (!rows.length) return el('div');
  group.append(...rows);
  return group;
}

// --- phone control (on the box's screen) --------------------------------------------

function phoneSettings() {
  const group = el('div', { class: 'settings-group' }, el('div', { class: 'section-title', text: t('Phone control') }));
  const list = el('div', { class: 'rows' });
  const draw = async () => {
    const devices = await get('/api/remote/devices').catch(() => []);
    list.replaceChildren(...devices.map((d) => el('div', { class: 'row' },
      el('div', { class: 'main' },
        el('div', { text: d.name }),
        el('div', { class: 'sub muted', text: day(d.paired_at) })),
      el('button', { class: 'secondary', text: t('Remove'), onclick: async () => {
        await post('/api/remote/forget', { id: d.id }).catch(() => null);
        draw();
      } }))));
  };
  draw();
  // On a phone that is not yet running the page from its home screen: how to put it there.
  const standalone = navigator.standalone || window.matchMedia('(display-mode: standalone)').matches;
  group.append(
    el('p', { class: 'muted small', text: t('Phones on this network can control the box from their browser once paired.') }),
    onPhone() && !standalone ? el('p', { class: 'muted small', text: /iPhone|iPad/.test(navigator.userAgent)
      ? t('To keep the box as an app on this iPhone: Share → Add to Home Screen. It pairs itself on its first start, within a day.')
      : t('To keep the box on this phone\'s home screen: the browser menu → Add to Home screen.') }) : '', // '' not null: append writes null out as text
    el('div', { class: 'buttons' }, el('button', { class: 'secondary', text: t('Pair a phone'), onclick: () => push(pairView()) })),
    list);
  return group;
}

function pairView() {
  return {
    async render(box) {
      const again = () => { box.replaceChildren(); this.render(box); };
      const r = await post('/api/remote/code');
      const left = el('p', { class: 'muted' });
      let until = Date.now() + r.expires_in * 1000;
      const tick = () => {
        const secs = Math.max(0, Math.round((until - Date.now()) / 1000));
        left.textContent = secs ? t('Valid for {t}', { t: clock(secs) }) : t('The code has expired.');
        if (secs && stack[stack.length - 1] === view) setTimeout(tick, 1000);
      };
      const view = stack[stack.length - 1];
      box.append(
        el('h2', { text: t('Pair a phone') }),
        el('p', { class: 'wiz-intro', text: t('Scan the code with the phone. It opens the box in its browser, ready to control it.') }),
        el('div', { class: 'wiz-signin' },
          el('img', { class: 'wiz-qr', src: `/api/remote/qr.svg?url=${encodeURIComponent(r.url)}`, alt: '', width: 220, height: 220 }),
          el('div', {},
            el('p', { class: 'link', text: r.url.replace(/\?.*$/, '') }),
            el('p', { class: 'code', text: r.code }),
            left,
            el('button', { class: 'secondary', text: t('New code'), onclick: again }))));
      tick();
    },
  };
}

function settingsView() {
  return {
    async render(box) {
      const dd = (id) => el('dd', { id });
      box.append(
        brand(44),
        el('dl', {},
          el('dt', { text: t('Account') }), dd('account'),
          el('dt', { text: t('Player') }), dd('mpv'),
          el('dt', { text: t('Audio') }), dd('audio'),
          el('dt', { text: t('Server') }), dd('server'),
          el('dt', { text: t('Power') }), dd('power'),
          el('dt', { text: t('Board') }), dd('board'),
          el('dt', { text: t('Display') }), dd('display'),
          el('dt', { text: t('Version') }), dd('version'),
          el('dt', { text: t('Running for') }), dd('uptime')),
        el('div', { class: 'buttons' },
          el('button', { id: 'tone', text: t('Play test tone'), onclick: testTone }),
          el('button', { id: 'signout', class: 'secondary', text: t('Sign out'), onclick: signOut })),
        el('p', { id: 'message' }));
      if (REMOTE) {
        // From a phone: what plays and how it sounds; the box's own settings stay on its screen.
        box.querySelector('#signout').remove();
        box.append(el('div', { class: 'settings-group' },
          el('div', { class: 'setting' }, el('span', { text: t('Sound') }),
            el('button', { class: 'secondary', text: t('Equalizer'), onclick: () => push(eqView()) }))));
        box.append(appearanceSettings());
        setTimeout(refreshStatus, 0);
        return;
      }
      box.append(nameSettings());
      box.append(languageSettings());
      box.append(appearanceSettings());
      box.append(phoneSettings());
      box.append(el('div', { class: 'settings-group' },
        el('div', { class: 'setting' }, el('span', { text: t('Sound') }),
          el('button', { class: 'secondary', text: t('Equalizer'), onclick: () => push(eqView()) }))));
      box.append(await outputSettings());
      box.append(await hatSettings());
      box.append(await sourcesSettings());
      box.append(serverSettings());
      box.append(await networkSettings());
      box.append(await updateSettings());
      const s = await get('/api/screen');
      if (s.controllable) box.append(screenSettings(s));
      // The rows are filled once the page is on screen, not 5 s later at the next refresh.
      setTimeout(refreshStatus, 0);
    },
  };
}

const OFF_AFTER = [[15, '15 s'], [30, '30 s'], [60, '1 min'], [300, '5 min'], [0, 'Never']];

function screenSettings(s) {
  const pct = (b) => `${Math.round((b / 255) * 100)} %`;
  const value = el('span', { class: 'muted', text: pct(s.brightness) });
  // One slider step per step of the panel (Touch Display 2 has 31), so every move shows.
  const step = s.levels && s.levels < 255 ? 255 / s.levels : 1;
  const min = s.levels && s.levels < 255 ? step * Math.max(1, Math.round(s.min_brightness / step)) : s.min_brightness;
  const slider = el('input', { type: 'range', min, max: 255, step, value: s.brightness, class: 'slider' });
  let pending = null;
  // Applied while dragging, at most a few times a second, so the panel follows the finger.
  slider.addEventListener('input', () => {
    value.textContent = pct(Number(slider.value));
    clearTimeout(pending);
    pending = setTimeout(() => post('/api/screen/settings', { brightness: Math.round(Number(slider.value)) }).catch(() => null), 120);
  });

  const choices = el('div', { class: 'buttons' });
  const draw = (current) => {
    choices.replaceChildren(...OFF_AFTER.map(([secs, label]) => el('button', {
      class: secs === current ? 'on' : 'secondary',
      text: t(label),
      onclick: async () => {
        const r = await post('/api/screen/settings', { off_after_secs: secs }).catch(() => null);
        screen.offAfter = r ? r.off_after_secs : secs;
        armIdle();
        draw(screen.offAfter);
      },
    })));
  };
  draw(s.off_after_secs);

  return el('div', { class: 'settings-group' },
    el('div', { class: 'section-title', text: t('Screen') }),
    el('div', { class: 'setting' }, el('span', { text: t('Brightness') }), slider, value),
    el('div', { class: 'setting' }, el('span', { text: t('Turn off after') }), choices),
    el('p', { class: 'muted small', text: t('The backlight draws the most power. On a weak supply, a bright screen that stays on can make the sound crackle.') }));
}

// --- settings: status, test tone, sign-out ---------------------------------------------

function show(id, text, cls) {
  const node = $(id);
  if (!node) return;
  node.textContent = text;
  node.className = cls || '';
}

function powerText(p) {
  if (!p) return [t('not available'), ''];
  if (p.under_voltage_now) return [t('under-voltage now ({r})', { r: p.raw }), 'bad'];
  if (p.throttled_now) return [t('throttled now ({r})', { r: p.raw }), 'bad'];
  if (p.under_voltage_since_boot) return [t('under-voltage since boot ({r})', { r: p.raw }), 'warn'];
  return [t('fine'), 'ok'];
}

function uptime(secs) {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return h ? `${h} h ${m} min` : `${m} min ${secs % 60} s`;
}

async function refreshStatus() {
  if (!$('mpv')) return;
  try {
    const s = await get('/api/status');
    if (account) show('account', account.display_name ? `${account.display_name} (${account.email})` : account.email, 'ok');
    if (s.mpv.running) show('mpv', s.mpv.version || 'mpv', 'ok');
    else show('mpv', t('not running: {e}', { e: s.mpv.error }), 'bad');
    show('audio', s.audio_device);
    show('server', s.server_url);
    show('power', ...powerText(s.power));
    const hw = s.hardware || {};
    const heat = [hw.temperature_c != null ? `${hw.temperature_c.toLocaleString(locale())} °C` : null, hw.cpu_mhz ? `${hw.cpu_mhz} MHz` : null].filter(Boolean).join(' · ');
    show('board', [hw.board || '—', heat].filter(Boolean).join(' · '));
    show('display', hw.display ? `${hw.display.name} (${hw.display.mode})` : '—');
    show('version', `ownbox ${s.version}`);
    show('uptime', uptime(s.uptime_secs));
  } catch (e) {
    show('mpv', e.message, 'bad');
  }
}

async function testTone() {
  show('message', t('Playing…'));
  try {
    await post('/api/test-tone');
    show('message', t('Test tone sent to the speaker.'));
  } catch (e) {
    show('message', t('Failed: {e}', { e: e.message }), 'bad');
  }
}

// Signing out takes two taps, so a stray touch can't do it.
let signoutArmed = null;
async function signOut() {
  const b = $('signout');
  if (!signoutArmed) {
    b.textContent = t('Tap again to sign out');
    signoutArmed = setTimeout(() => {
      signoutArmed = null;
      if ($('signout')) $('signout').textContent = t('Sign out');
    }, 3000);
    return;
  }
  clearTimeout(signoutArmed);
  signoutArmed = null;
  await post('/api/sign-out').catch(() => null);
  refreshSession();
}

// --- now playing ------------------------------------------------------------------

let now = null;

function markCurrent() {
  const id = now && now.track ? now.track.id : null;
  document.querySelectorAll('#view .row[data-id]').forEach((r) => r.classList.toggle('current', r.dataset.id === id));
}

// AirPlay or Music Assistant playing to the box: shown in the bar and on the now-playing
// screen in place of the box's own track; play takes the card back for own.audio.
let outsideShown = null;

function renderOutside(o) {
  const key = `${o.kind}:${o.title}:${o.artist}:${o.cover_version}:${o.sender}`;
  $('nowbar').hidden = false;
  ['nb-prev', 'nb-next', 'np-prev', 'np-next', 'np-back30', 'np-fwd30'].forEach((id) => { $(id).hidden = true; });
  document.querySelector('.np-book').hidden = true;
  setIcon($('nb-toggle'), 'play');
  setIcon($('np-toggle'), 'play');
  $('nb-progress').firstElementChild.style.width = '0%';
  $('np-bar').firstElementChild.style.width = '0%';
  if (outsideShown === key) return;
  outsideShown = key;
  const source = o.kind === 'airplay' ? 'AirPlay' : 'Music Assistant';
  const from = [source, o.sender].filter(Boolean).join(' · ');
  for (const img of [$('nb-cover'), $('np-cover'), $('np-glow-img')]) {
    if (o.cover_version) img.src = `/api/outside/cover?v=${o.cover_version}`;
    else img.removeAttribute('src');
  }
  $('nb-title').textContent = o.title || source;
  $('nb-artist').textContent = [o.artist, from].filter(Boolean).join(' · ');
  $('np-title').textContent = o.title || source;
  $('np-artist').textContent = o.artist || '';
  $('np-album').textContent = from;
  $('np-pos').textContent = '';
  $('np-dur').textContent = '';
  actify($('np-queue'), 'back', t('Back to own.audio'));
  $('np-add').hidden = true;
}

function renderNow(n, o) {
  if (o) {
    now = n;
    return renderOutside(o);
  }
  if (outsideShown) {
    // Back from an outside source: show the box's own track again in full.
    outsideShown = null;
    ['nb-prev', 'nb-next', 'np-prev', 'np-next'].forEach((id) => { $(id).hidden = false; });
    now = null;
  }
  const prevId = now && now.track ? now.track.id : null;
  now = n;
  const tk = n.track;
  if ($('nowbar').hidden !== !tk) {
    $('nowbar').hidden = !tk;
    if (scrub.onJump) placeScrub();
  }
  if (!tk) {
    $('nowplaying').hidden = true;
    return;
  }
  const playing = n.state === 'playing';
  $('np-add').hidden = !!(tk.book_id || tk.podcast_id);
  const frac = n.duration_secs ? Math.min(1, n.position_secs / n.duration_secs) : 0;
  if (tk.id !== prevId) {
    // No src at all rather than an empty one: an empty src draws a broken-image icon.
    for (const img of [$('nb-cover'), $('np-cover'), $('np-glow-img')]) {
      if (tk.image) img.src = tk.image;
      else img.removeAttribute('src');
    }
    $('nb-title').textContent = tk.book_id ? (tk.album || tk.title) : tk.title;
    $('nb-artist').textContent = tk.book_id && n.queue_length > 1 ? `${tk.artist} · ${t('part {i} of {n}', { i: n.index + 1, n: n.queue_length })}` : tk.artist;
    // A book: its title first, the part underneath; a song: title, artist, album.
    $('np-title').textContent = tk.book_id ? (tk.album || tk.title) : tk.title;
    $('np-artist').textContent = tk.artist;
    $('np-album').textContent = tk.book_id
      ? (n.queue_length > 1 ? t('Part {i} of {n}', { i: n.index + 1, n: n.queue_length }) : '')
      : (tk.album || '');
    markCurrent();
  }
  setIcon($('nb-toggle'), playing ? 'pause' : 'play');
  setIcon($('np-toggle'), playing ? 'pause' : 'play');
  $('nb-progress').firstElementChild.style.width = `${frac * 100}%`;
  $('np-bar').firstElementChild.style.width = `${frac * 100}%`;
  $('np-pos').textContent = clock(n.position_secs);
  $('np-dur').textContent = clock(n.duration_secs);
  actify($('np-queue'), 'queue', n.queue_length > 1 ? t('Queue · {i} of {n}', { i: n.index + 1, n: n.queue_length }) : t('Queue'));
  // Don't move the slider under a finger that is dragging it.
  if (!volumeDragging) $('np-volume').value = Math.round(n.volume);
  // Books and podcasts: ±30 s, speed and the sleep timer.
  const spoken = n.kind !== 'music';
  $('np-back30').hidden = !spoken;
  $('np-fwd30').hidden = !spoken;
  document.querySelector('.np-book').hidden = !spoken;
  const sp = Math.round(n.speed * 100) / 100;
  $('np-speed').textContent = `${Number.isInteger(sp) ? sp.toFixed(1) : sp}×`;
  const sleeping = n.sleep_end_of_part || n.sleep_in_secs != null;
  $('np-sleep').classList.toggle('on', sleeping);
  $('np-sleep').classList.toggle('secondary', !sleeping);
  actify($('np-sleep'), 'sleep', n.sleep_end_of_part ? (n.kind === 'podcast' ? t('Sleep: end of episode') : t('Sleep: end of part'))
    : n.sleep_in_secs != null ? t('Sleep in {m} min', { m: Math.ceil(n.sleep_in_secs / 60) }) : t('Sleep timer'));
}

// Books: ±30 s, speed and a sleep timer, each one tap that steps through its choices.
const SPEEDS = [0.8, 1.0, 1.2, 1.5, 1.75, 2.0];
$('np-back30').addEventListener('click', () => control('skip', { secs: -30 }));
$('np-fwd30').addEventListener('click', () => control('skip', { secs: 30 }));
$('np-speed').addEventListener('click', () => {
  const i = SPEEDS.findIndex((x) => Math.abs(x - (now ? now.speed : 1)) < 0.01);
  control('speed', { speed: SPEEDS[(i + 1) % SPEEDS.length] });
});
$('np-sleep').addEventListener('click', () => {
  // off → 15 → 30 → 60 min → end of part → off
  let next;
  if (now && now.sleep_end_of_part) next = {};
  else if (now && now.sleep_in_secs != null) {
    const m = now.sleep_in_secs / 60;
    next = m > 30 ? { end_of_part: true } : m > 15 ? { minutes: 60 } : { minutes: 30 };
  } else next = { minutes: 15 };
  control('sleep', next);
});

// --- volume and queue -------------------------------------------------------------

let volumeDragging = false;
let volumeTimer = null;
$('np-vol-icon').innerHTML = icon('volume');
$('np-volume').addEventListener('pointerdown', () => { volumeDragging = true; });
$('np-volume').addEventListener('pointerup', () => { volumeDragging = false; });
$('np-volume').addEventListener('input', () => {
  clearTimeout(volumeTimer);
  volumeTimer = setTimeout(() => post('/api/player/volume', { volume: Number($('np-volume').value) }).catch(() => null), 100);
});

async function openQueue() {
  const q = await get('/api/player/queue').catch(() => null);
  if (!q) return;
  const list = $('queue-list');
  list.replaceChildren(...q.tracks.map((t, i) => {
    const row = trackRow(t, i + 1, async () => {
      await post('/api/player/jump', { index: i }).catch(() => null);
      $('queue').hidden = true;
      pollNow();
    }, artistAndAlbum(t));
    if (i === q.index && q.active) row.classList.add('current');
    return row;
  }));
  $('queue').hidden = false;
  const current = list.children[q.index];
  if (current) current.scrollIntoView({ block: 'center' });
}

// While an outside source plays, the same button brings own.audio back instead.
$('np-queue').addEventListener('click', () => (outsideShown ? control('toggle') : openQueue()));
$('np-eq').addEventListener('click', () => {
  $('nowplaying').hidden = true;
  push(eqView());
});
setIcon($('queue-close'), 'down');
$('queue-close').addEventListener('click', () => { $('queue').hidden = true; });

// Controls on their way to the box: a poll answered meanwhile would show the old state
// and undo the play/pause icon flipped at the tap.
let controlling = 0;

async function pollNow() {
  if (!signedIn) return;
  try {
    const [n, o] = await Promise.all([get('/api/player'), get('/api/outside').catch(() => null)]);
    if (controlling) return;
    renderNow(n, o);
  } catch (e) {
    // ownbox restarting; the next poll will tell
  }
}

async function control(what, body) {
  controlling += 1;
  try {
    await post(`/api/player/${what}`, body);
  } catch (e) {
    // "this is the last track" and the like: nothing to show on a tap
  } finally {
    controlling -= 1;
  }
  pollNow();
}

// Play/pause flips at the tap; the next poll confirms it or puts it back.
function toggleNow() {
  if (now && now.track && !outsideShown) {
    // Kept in `now` too, so a second quick tap flips it back.
    now.state = now.state === 'playing' ? 'paused' : 'playing';
    const icon = now.state === 'playing' ? 'pause' : 'play';
    setIcon($('nb-toggle'), icon);
    setIcon($('np-toggle'), icon);
  }
  control('toggle');
}

['prev', 'next'].forEach((k) => {
  setIcon($(`nb-${k}`), k);
  setIcon($(`np-${k}`), k);
});
setIcon($('np-close'), 'down');
$('nb-prev').addEventListener('click', () => control('previous'));
$('np-prev').addEventListener('click', () => control('previous'));
$('nb-next').addEventListener('click', () => control('next'));
$('np-next').addEventListener('click', () => control('next'));
$('np-add').addEventListener('click', () => { if (now && now.track) openPicker([now.track.id]); });
$('nb-toggle').addEventListener('click', toggleNow);
$('np-toggle').addEventListener('click', toggleNow);
$('nb-open').addEventListener('click', () => { $('nowplaying').hidden = false; });
$('np-close').addEventListener('click', () => { $('nowplaying').hidden = true; });
$('np-bar').addEventListener('click', (e) => {
  if (!now || !now.duration_secs) return;
  const box = e.currentTarget.getBoundingClientRect();
  const frac = Math.min(1, Math.max(0, (e.clientX - box.left) / box.width));
  control('seek', { secs: frac * now.duration_secs });
});

// --- backlight --------------------------------------------------------------------
// The backlight goes off after a while without a touch (it is what overloads the supply).
// The touch panel keeps working in the dark; the first touch only wakes the screen and is
// swallowed, so nothing invisible gets pressed.

const screen = { on: true, offAfter: 0, timer: null, swallowUntil: 0 };

function setScreen(on) {
  screen.on = on;
  post('/api/screen', { on }).catch(() => null);
}

// ownbox turns a screen off itself when the page has not reported a touch for a while
// (see screen.rs); tell it, at most every 5 s.
let lastTouchSent = 0;
function reportTouch() {
  if (Date.now() - lastTouchSent < 5000) return;
  lastTouchSent = Date.now();
  post('/api/screen/touch').catch(() => null);
}

function armIdle() {
  clearTimeout(screen.timer);
  if (!screen.offAfter) return;
  screen.timer = setTimeout(() => {
    // A sign-in code on screen is being read off it; leave it lit.
    const keepLit = (!$('signin').hidden && !$('signin-pending').hidden) || document.body.classList.contains('wizard');
    if (keepLit) {
      lastTouchSent = 0;
      reportTouch();
      return armIdle();
    }
    setScreen(false);
  }, screen.offAfter * 1000);
}

function onTouch(e) {
  if (!screen.on) {
    setScreen(true);
    screen.swallowUntil = Date.now() + 800;
    e.preventDefault();
    e.stopPropagation();
  }
  reportTouch();
  armIdle();
}

function swallow(e) {
  if (Date.now() < screen.swallowUntil) {
    e.preventDefault();
    e.stopPropagation();
  }
}

/** Only on the box's own screen: a phone must not switch the box's backlight. */
function startScreenTimer() {
  document.addEventListener('pointerdown', onTouch, true);
  document.addEventListener('click', swallow, true);
  document.addEventListener('pointerup', swallow, true);
  const sync = () => get('/api/screen')
    .then((s) => {
      const wasOn = screen.on;
      screen.on = s.on;
      screen.offAfter = s.controllable ? s.off_after_secs : 0;
      // Lit by someone else (ownbox restarting): start counting from now.
      if (s.on && !wasOn) armIdle();
    })
    .catch(() => null);
  sync().then(armIdle);
  // ownbox may change the screen behind the page's back (a restart lights it, the
  // forgotten-screen timer darkens it); keep the page's idea of it true, so the first touch
  // on a dark screen only wakes it.
  setInterval(sync, 3000);
}

// --- start ------------------------------------------------------------------------

// A phone (not the box's own screen) is a remote: it pairs first, and leaves the box's
// administration to the screen.
let REMOTE = false;
// A box without a screen (the radio): a paired phone is its screen.
let HEADLESS = false;

function phoneName() {
  const ua = navigator.userAgent;
  return /iPhone/.test(ua) ? 'iPhone' : /iPad/.test(ua) ? 'iPad' : /Android/.test(ua) ? 'Android' : 'Browser';
}

function pairingHelp(error) {
  $('app').hidden = true;
  $('signin').hidden = false;
  $('signin-pending').hidden = true;
  $('signin-start').hidden = false;
  $('signin-button').hidden = true;
  document.querySelector('#signin-start h2').textContent = t('Pair this phone');
  document.querySelector('#signin-start p').textContent = HEADLESS
    ? t('This box has no screen. On a phone that already controls it, open Settings → Phone control → Pair a phone, and scan the code with this phone.')
    : t('On the box, open Settings → Phone control → Pair a phone, and scan the code with this phone.');
  $('signin-error').textContent = error || '';
}

// --- the setup Wi-Fi ------------------------------------------------------------------
// A box without a screen or a network opens a Wi-Fi of its own; a phone that joins gets this
// page by itself. A few short steps — welcome, the home Wi-Fi, a name, the sound — then the
// box joins the home network, so the last page cannot follow it: it says where to go next.
// It follows the phone's own light or dark setting, and needs nothing from the internet.

const WIFI_BARS = (signal) => {
  const lit = signal >= 70 ? 4 : signal >= 45 ? 3 : signal >= 25 ? 2 : 1;
  const bar = (i, h) => `<rect x="${2 + i * 5}" y="${18 - h}" width="3.4" height="${h}" rx="1.2" opacity="${i < lit ? 1 : 0.25}"/>`;
  return `<svg viewBox="0 0 22 20" width="22" height="20" aria-hidden="true" fill="currentColor">${bar(0, 5)}${bar(1, 9)}${bar(2, 13)}${bar(3, 17)}</svg>`;
};
const OB_ICON = {
  check: '<path d="M5 12.5 10 17.5 19 7" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"/>',
  speaker: '<rect x="6" y="3" width="12" height="18" rx="3" fill="none" stroke="currentColor" stroke-width="2"/><circle cx="12" cy="14.5" r="3.2" fill="none" stroke="currentColor" stroke-width="2"/><circle cx="12" cy="7.5" r="1.3"/>',
  chip: '<rect x="5" y="5" width="14" height="14" rx="2.5" fill="none" stroke="currentColor" stroke-width="2"/><path d="M9 2v3M15 2v3M9 19v3M15 19v3M2 9h3M2 15h3M19 9h3M19 15h3" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>',
  radio: '<rect x="3" y="8" width="18" height="12" rx="3" fill="none" stroke="currentColor" stroke-width="2"/><circle cx="15.5" cy="14" r="2.8" fill="none" stroke="currentColor" stroke-width="2"/><path d="M6.5 12.5h4M6.5 15.5h4M7 8l9-5" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>',
  eye: '<path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z" fill="none" stroke="currentColor" stroke-width="2"/><circle cx="12" cy="12" r="3"/>',
  globe: '<circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" stroke-width="2"/><path d="M3 12h18M12 3c3 3.2 3 14.8 0 18M12 3c-3 3.2-3 14.8 0 18" fill="none" stroke="currentColor" stroke-width="1.8"/>',
  cloud: '<path d="M7 18.5h10.5a4 4 0 0 0 .6-7.95A6 6 0 0 0 6.5 9.6 4.5 4.5 0 0 0 7 18.5z" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"/>',
  server: '<rect x="4" y="3.5" width="16" height="7" rx="2" fill="none" stroke="currentColor" stroke-width="2"/><rect x="4" y="13.5" width="16" height="7" rx="2" fill="none" stroke="currentColor" stroke-width="2"/><circle cx="8" cy="7" r="1.2"/><circle cx="8" cy="17" r="1.2"/>',
  wifi: '<path d="M2.5 9a14 14 0 0 1 19 0M5.5 12.5a9.5 9.5 0 0 1 13 0M8.6 15.8a5 5 0 0 1 6.8 0" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><circle cx="12" cy="19" r="1.5"/>',
};
const obIcon = (name, size = 24) => {
  const span = el('span', { class: 'ob-icon', 'aria-hidden': 'true' });
  span.innerHTML = `<svg viewBox="0 0 24 24" width="${size}" height="${size}">${OB_ICON[name]}</svg>`;
  return span;
};

// A sound card's label in two lines: "Pimoroni pHAT BEAT (Pirate Radio)" → its name, and
// what it is in or what it covers underneath.
const obTitle = (label) => t(label).replace(/\s*\(.*\)\s*$/, '');
const obSub = (label) => (t(label).match(/\((.*)\)\s*$/) || [])[1] || '';

// The address the box takes from its name, as `hotspot::host_name` makes it.
function obHostName(name) {
  const h = name.toLowerCase().replace(/own\.audio/g, '').normalize('NFKD').replace(/[̀-ͯ]/g, '')
    .replace(/ł/g, 'l').replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 40).replace(/-+$/, '');
  return !h || h === 'box' ? 'ownbox' : h;
}

async function setupWifiPage() {
  $('signin').hidden = true;
  $('app').hidden = true;
  document.documentElement.dataset.theme = 'auto';
  const page = el('section', { id: 'setup', class: 'setup ob' });
  document.body.append(page);
  const st = await get('/api/hotspot');
  if (st.language === 'cs' || st.language === 'en') LANG = st.language;

  const STEPS = ['welcome', 'wifi', 'name', 'sound', 'check'];
  const form = {
    ssid: null,
    password: '',
    name: st.box_name || 'own.audio box',
    hat: st.hats.current != null ? st.hats.current : 0,
    // A network that does not send its name: typed, not picked.
    hidden: false,
  };
  // Back from a try that failed: straight to the network, with the reason.
  let step = st.error ? 'wifi' : 'welcome';
  let showPassword = false;

  const network = () => st.networks.find((n) => n.ssid === form.ssid);
  const needsPassword = () => !!(network() && network().secure);
  const canGo = {
    welcome: () => true,
    wifi: () => (form.hidden
      ? form.ssid && form.ssid.trim().length > 0 && (form.password.length === 0 || form.password.length >= 8)
      : !!form.ssid && (!needsPassword() || form.password.length >= 8)),
    name: () => form.name.trim().length > 0,
    sound: () => true,
    check: () => true,
  };
  const go = (to) => { step = to; draw(); page.scrollTop = 0; };
  const next = () => go(STEPS[STEPS.indexOf(step) + 1]);
  const back = () => go(STEPS[STEPS.indexOf(step) - 1]);

  const head = () => {
    const i = STEPS.indexOf(step);
    return el('header', { class: 'ob-head' },
      i > 0 ? el('button', { class: 'icon ob-back', 'aria-label': t('Back'), onclick: back }) : el('span', { class: 'ob-back' }),
      el('div', { class: 'ob-progress', role: 'progressbar', 'aria-valuemin': 1, 'aria-valuemax': STEPS.length, 'aria-valuenow': i + 1 },
        ...STEPS.map((_, j) => el('span', { class: j <= i ? 'on' : '' }))),
      el('div', { class: 'ob-lang' },
        ...[['cs', 'CZ'], ['en', 'EN']].map(([code, label]) => el('button', {
          class: LANG === code ? 'on' : '', text: label, 'aria-pressed': String(LANG === code),
          onclick: () => { LANG = code; draw(); },
        }))));
  };
  const foot = (label, onclick, enabled = true) => el('footer', { class: 'ob-foot' },
    el('button', { class: 'primary ob-go', text: label, disabled: !enabled, onclick }));

  const views = {
    welcome: () => [
      el('div', { class: 'ob-hero' },
        (() => { const b = brand(96, { animated: true, big: true }); b.classList.add('ob-brand'); return b; })(),
        el('h1', { text: t('Your music, from your own library') }),
        el('p', { class: 'ob-lead', text: t('Three short steps and this box joins your home Wi-Fi. Then it plays from your own.audio library.') })),
      el('ol', { class: 'ob-list' },
        ...[t('Your Wi-Fi'), t('A name for the box'), t('Its sound')].map((s, i) => el('li', {},
          el('span', { class: 'ob-num', text: String(i + 1) }), el('span', { text: s })))),
      foot(t('Start'), next),
    ],

    wifi: () => {
      const password = el('input', {
        type: showPassword ? 'text' : 'password', value: form.password, autocomplete: 'off', autocapitalize: 'off',
        spellcheck: 'false', placeholder: t('Wi-Fi password'), class: 'ob-input',
        oninput: (e) => { form.password = e.target.value; nextBtn.disabled = !canGo.wifi(); },
        onkeydown: (e) => { if (e.key === 'Enter' && canGo.wifi()) next(); },
      });
      const eye = el('button', {
        class: 'icon ob-eye' + (showPassword ? ' on' : ''), 'aria-label': t('Show the password'), 'aria-pressed': String(showPassword),
        onclick: () => { showPassword = !showPassword; password.type = showPassword ? 'text' : 'password'; eye.classList.toggle('on', showPassword); },
      }, obIcon('eye'));
      const card = (n) => {
        const chosen = !form.hidden && n.ssid === form.ssid;
        const bars = el('span', { class: 'ob-bars' });
        bars.innerHTML = WIFI_BARS(n.signal);
        return el('div', { class: chosen ? 'ob-net chosen' : 'ob-net' },
          el('button', {
            class: 'ob-net-pick', 'aria-pressed': String(chosen),
            onclick: () => { if (form.ssid !== n.ssid || form.hidden) { form.ssid = n.ssid; form.password = ''; form.hidden = false; } draw(); if (n.secure) setTimeout(() => $('ob-pass')?.focus(), 50); },
          },
          bars,
          el('span', { class: 'ob-net-name ellipsis', text: n.ssid }),
          el('span', { class: 'ob-net-sub', text: n.secure ? '' : t('open') }),
          chosen ? obIcon('check', 22) : null),
          chosen && n.secure ? el('div', { class: 'ob-pass' }, Object.assign(password, { id: 'ob-pass' }), eye) : null,
          chosen && n.secure && form.password.length > 0 && form.password.length < 8
            ? el('p', { class: 'ob-hint', text: t('A Wi-Fi password has at least 8 characters.') }) : null);
      };
      const nextBtn = el('button', { class: 'primary ob-go', text: t('Continue'), disabled: !canGo.wifi(), onclick: next });
      // The last row: a network not in the list because it does not send its name.
      function hiddenCard() {
        const open = form.hidden;
        const name = el('input', { type: 'text', class: 'ob-input', value: open ? (form.ssid || '') : '', placeholder: t('Network name'),
          autocomplete: 'off', autocapitalize: 'off', spellcheck: 'false',
          oninput: (e) => { form.ssid = e.target.value; nextBtn.disabled = !canGo.wifi(); } });
        const pass = el('input', { type: 'password', class: 'ob-input', value: open ? form.password : '', placeholder: t('Password, if it has one'),
          autocomplete: 'off', oninput: (e) => { form.password = e.target.value; nextBtn.disabled = !canGo.wifi(); } });
        return el('div', { class: open ? 'ob-net chosen' : 'ob-net' },
          el('button', { class: 'ob-net-pick', 'aria-pressed': String(open),
            onclick: () => { if (!form.hidden) { form.hidden = true; form.ssid = ''; form.password = ''; } draw(); setTimeout(() => name.focus(), 50); } },
          obIcon('wifi', 22), el('span', { class: 'ob-net-name', text: t('Another network (hidden)') }), open ? obIcon('check', 22) : null),
          open ? el('div', { class: 'ob-hidden' }, name, pass) : null);
      }
      return [
        el('h2', { text: t('Which Wi-Fi should the box join?') }),
        el('p', { class: 'ob-lead', text: t('Choose your home network. The box only knows 2.4 GHz networks.') }),
        st.error ? el('div', { class: 'ob-alert', role: 'alert' },
          el('strong', { text: t('The box could not join.') }), ' ', t(st.error)) : null,
        st.networks.length
          ? el('div', { class: 'ob-nets' }, ...st.networks.map(card), hiddenCard())
          : el('div', { class: 'ob-empty' }, el('p', { text: t('No networks in reach.') }),
            el('p', { class: 'muted', text: t('Move the box closer to your router; it looks again in a few minutes.') }),
            el('div', { class: 'ob-nets' }, hiddenCard())),
        el('footer', { class: 'ob-foot' }, nextBtn),
      ];
    },

    name: () => {
      const address = el('span', { class: 'ob-addr', text: `${obHostName(form.name)}.local` });
      const input = el('input', {
        type: 'text', value: form.name, maxlength: 60, autocomplete: 'off', class: 'ob-input big',
        oninput: (e) => { form.name = e.target.value; address.textContent = `${obHostName(form.name)}.local`; nextBtn.disabled = !canGo.name(); },
        onkeydown: (e) => { if (e.key === 'Enter' && canGo.name()) next(); },
      });
      const rooms = LANG === 'cs' ? ['Obývák', 'Kuchyň', 'Ložnice', 'Dětský pokoj', 'Pracovna'] : ['Living room', 'Kitchen', 'Bedroom', 'Kids\' room', 'Study'];
      const nextBtn = el('button', { class: 'primary ob-go', text: t('Continue'), disabled: !canGo.name(), onclick: next });
      return [
        el('h2', { text: t('What should we call it?') }),
        el('p', { class: 'ob-lead', text: t('The name shows in AirPlay and in your own.audio apps, and it becomes the box\'s address.') }),
        input,
        el('div', { class: 'ob-chips' }, ...rooms.map((r) => el('button', {
          class: 'secondary', text: r, onclick: () => { form.name = r; input.value = r; input.dispatchEvent(new Event('input')); },
        }))),
        el('p', { class: 'ob-note' }, t('Address: '), address),
        el('footer', { class: 'ob-foot' }, nextBtn),
      ];
    },

    sound: () => [
      el('h2', { text: t('Where does the sound come out?') }),
      el('p', { class: 'ob-lead', text: t('A sound card on the pins must be named here; USB, HDMI and the headphone jack are found by themselves.') }),
      el('div', { class: 'ob-choices', role: 'radiogroup' }, ...st.hats.choices.map((c) => {
        const chosen = form.hat === c.id;
        const icon = c.id === 0 ? 'speaker' : c.buttons ? 'radio' : 'chip';
        return el('button', {
          class: chosen ? 'ob-choice chosen' : 'ob-choice', role: 'radio', 'aria-checked': String(chosen),
          onclick: () => { form.hat = c.id; draw(); },
        }, obIcon(icon, 26), el('span', { class: 'ob-choice-text' },
          el('span', { class: 'ob-choice-title', text: obTitle(c.label) }),
          obSub(c.label) ? el('span', { class: 'ob-choice-sub', text: obSub(c.label) }) : null), chosen ? obIcon('check', 22) : null);
      })),
      foot(t('Continue'), next),
    ],

    check: () => {
      const hat = st.hats.choices.find((c) => c.id === form.hat);
      const row = (label, value, to) => el('button', { class: 'ob-sum-row', onclick: () => go(to) },
        el('span', { class: 'muted', text: label }), el('span', { class: 'ob-sum-val ellipsis', text: value }));
      const msg = el('p', { class: 'ob-hint bad', role: 'alert' });
      const goBtn = el('button', {
        class: 'primary ob-go', text: t('Connect the box'),
        onclick: async () => {
          goBtn.disabled = true;
          goBtn.classList.add('busy');
          try {
            const r = await post('/api/hotspot/apply', {
              ssid: form.ssid.trim(), password: form.password || null, name: form.name.trim(), language: LANG, hat: form.hat, hidden: form.hidden,
            });
            done(r);
          } catch (e) {
            msg.textContent = t(e.message);
            goBtn.disabled = false;
            goBtn.classList.remove('busy');
          }
        },
      });
      return [
        el('h2', { text: t('All set?') }),
        el('p', { class: 'ob-lead', text: t('Tap a line to change it.') }),
        el('div', { class: 'ob-sum' },
          row(t('Wi-Fi'), form.ssid, 'wifi'),
          row(t('Name'), form.name.trim(), 'name'),
          row(t('Sound'), hat ? obTitle(hat.label) : '', 'sound')),
        msg,
        el('footer', { class: 'ob-foot' }, goBtn),
      ];
    },
  };

  const done = (r) => {
    const link = `http://${r.address}/`;
    const address = el('a', { href: link, text: r.address });
    const copy = el('button', {
      class: 'secondary', text: t('Copy'),
      onclick: async () => {
        if (await copyText(link)) copy.textContent = t('Copied');
        else { selectText(address); copy.textContent = t('Hold the address to copy it'); }
      },
    });
    page.replaceChildren(el('div', { class: 'ob-body ob-done' },
      el('div', { class: 'ob-hero' },
        brand(96, { animated: true, big: true }),
        el('h1', { text: t('The box is joining {s}', { s: r.ssid }) })),
      el('ol', { class: 'ob-list' },
        el('li', {}, el('span', { class: 'ob-num', text: '1' }),
          el('span', { text: t('This Wi-Fi closes in a moment. Connect this phone back to {s}.', { s: r.ssid }) })),
        el('li', {}, el('span', { class: 'ob-num', text: '2' }),
          el('span', { text: t('Open the box\'s address. There it asks for your own.audio library.') }))),
      el('div', { class: 'ob-link' }, address, copy),
      el('p', { class: 'ob-note', text: t('If the box cannot join, its own Wi-Fi comes back within a minute, with the reason.') })));
  };

  const draw = () => {
    page.replaceChildren(head(), el('div', { class: `ob-body ob-${step}` }, ...views[step]().filter(Boolean)));
    const backBtn = page.querySelector('.ob-back');
    if (backBtn && backBtn.tagName === 'BUTTON') backBtn.innerHTML = `<svg viewBox="0 0 24 24">${ICONS.back}</svg>`;
  };
  draw();
  // NetworkManager keeps scanning and the box merges what it sees: while the network list
  // is on screen, it fills in — but never under a finger typing a password.
  setInterval(async () => {
    if (step !== 'wifi' || (document.activeElement && document.activeElement.tagName === 'INPUT')) return;
    const fresh = await get('/api/hotspot').catch(() => null);
    if (!fresh || JSON.stringify(fresh.networks) === JSON.stringify(st.networks)) return;
    st.networks = fresh.networks;
    const top = page.scrollTop;
    draw();
    page.scrollTop = top;
  }, 8000);
}

async function start() {
  document.querySelector('#signin header.brand').replaceChildren(brand(56));
  // Who is asking decides the whole page: a box that does not answer yet is asked again,
  // not taken to mean "this is the box's own screen".
  let rs = null;
  for (let i = 0; !rs; i += 1) {
    rs = await get('/api/remote/status').catch(() => null);
    if (!rs) {
      $('signin').hidden = false;
      $('signin-error').textContent = t('ownbox is not answering');
      await new Promise((ok) => setTimeout(ok, Math.min(5000, 1000 * (i + 1))));
    }
  }
  $('signin-error').textContent = '';
  if (rs.setup_wifi) {
    document.body.classList.add('phone');
    return setupWifiPage();
  }
  REMOTE = !rs.screen;
  HEADLESS = !!rs.headless;
  document.body.classList.toggle('kiosk', !REMOTE && !HEADLESS);
  document.body.classList.toggle('phone', REMOTE || HEADLESS);
  // The language comes first, so the first screen is already in it.
  const u = await get('/api/ui').catch(() => null);
  if (u) LANG = u.language === 'en' ? 'en' : 'cs';
  if (u) applyTheme(u.theme);
  applyButtons(u && u.buttons);
  translatePage();
  labelPlayer();
  if (REMOTE) {
    const code = new URLSearchParams(location.search).get('pair');
    if (!rs.paired && (code || rs.first)) {
      try {
        // A box without a screen takes its first phone without a code.
        await post('/api/remote/pair', code ? { code, name: phoneName() } : { first: true, name: phoneName() });
        location.replace('/');
      } catch (e) {
        pairingHelp(t(e.message));
      }
      return;
    }
    if (!rs.paired) return pairingHelp();
    setup = { done: true }; // setting the box up is the screen's job
  } else {
    setup = await get('/api/setup').catch(() => ({ done: true }));
    if (!HEADLESS) startScreenTimer();
  }
  if (!setup.done) startWizard();
  else refreshSession();
  setInterval(once(refreshSession), 2000);
  // A phone puts a tab in the background to sleep; back in front, it asks at once.
  document.addEventListener('visibilitychange', () => {
    if (document.hidden) return;
    refreshSession();
    pollNow();
    window.dispatchEvent(new Event('ownbox-wake'));
  });
  setInterval(once(pollNow), 1000);
  setInterval(once(refreshStatus), 5000);
  // A theme or button style chosen on another device (a phone) reaches this one too.
  setInterval(() => get('/api/ui').then((u) => {
    if (u.theme !== THEME) applyTheme(u.theme);
    if ((u.buttons || 'text') !== BUTTONS) applyButtons(u.buttons);
  }).catch(() => null), 5000);
}

start();
