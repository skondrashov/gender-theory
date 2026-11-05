// The editor: the real renderer runs in #stage (the wasm, `?editor`), and everything
// around it edits static/album.json — scenes, layers, hotspots and bands, all in bars of
// a track — pushing every change into the app live and saving through the dev server.
//
// The album's shape is documented in app/src/scenes/data.rs.

import init, { main_web, editor as send, state as readState, spectrum as readSpectrum, catalogue as readCatalogue } from './app/gender.js';

const W = 20, H = 20;
const STAGE = 500;
const CELL = 12.5;   // grid px per cell
const PPB = 12;      // timeline px per bar
const GUTTER = 72;   // timeline lane labels
const UNLOCKS = ['glow', 'lines', 'length', 'rotation', 'circles', 'color'];
const BAND_COLORS = ['#ff7333', '#ffd23f', '#3fd2a0', '#3f9fff', '#c77dff', '#ff5e8a', '#9cff57'];

const $ = (s) => document.querySelector(s);
const el = (tag, attrs = {}, ...children) => {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === 'class') node.className = v;
    else if (k.startsWith('on')) node.addEventListener(k.slice(2), v);
    else if (k === 'title' || k === 'type' || k === 'value' || k === 'step' || k === 'min' || k === 'max' || k === 'placeholder') node[k] = v;
    else node.setAttribute(k, v);
  }
  for (const c of children.flat()) if (c != null) node.append(c.nodeType ? c : String(c));
  return node;
};
const clamp = (x, a, b) => Math.min(b, Math.max(a, x));
const uid = (p) => p + Date.now().toString(36) + Math.random().toString(36).slice(2, 5);

let album, cat;
let ti = 0;                // index of the track being edited
let si = 0;                // index of the scene being edited (its layers and hotspots are shown)
let follow = true;         // si follows the scene playing, until a scene is picked by hand
let sel = null;            // {type: 'scene'|'layer'|'hotspot'|'band', index}
let st = {};               // last state from the app
let history = [], future = [];
let dirty = false;
let pushTimer = null;
const waves = {};          // track id → {peaks, bars, duration}
let audioCtx;

const track = () => album.tracks[ti];
const scene = () => track().scenes[si];
const allSceneIds = () => album.tracks.flatMap((t) => t.scenes.map((s) => s.id));
const trackBars = (t) => {
  const known = waves[t.id]?.bars ?? 0;
  const used = Math.max(0, ...t.scenes.map((s) => s.bars[1]));
  return Math.max(known, used, 16);
};
const sceneAt = (t, bar) => t.scenes.find((s) => s.bars[0] <= bar && bar < s.bars[1]);
const overlaps = (a, b) => a.bars[0] < b.bars[1] && b.bars[0] < a.bars[1];

// A scene's layers and hotspots live inside its bars: when the scene moves they move with
// it, when it shrinks they're clipped.
function fitScene(s, oldBars) {
  const shift = s.bars[0] - oldBars[0];
  for (const item of [...(s.layers || []), ...(s.hotspots || [])]) {
    if (shift && item.bars[1] - item.bars[0] === oldBars[1] - oldBars[0]) { item.bars = [...s.bars]; continue; }
    item.bars[0] = clamp(item.bars[0] + shift, s.bars[0], s.bars[1] - 1);
    item.bars[1] = clamp(item.bars[1] + shift, item.bars[0] + 1, s.bars[1]);
  }
}

// ---- boot ----

async function boot() {
  // The app reads its flags from this page's URL: it must know it's in the editor.
  if (!/[?&]editor/.test(location.search)) { location.replace(`editor.html?editor=${STAGE}`); return; }
  album = await (await fetch('album.json', { cache: 'no-store' })).json();
  await init();
  cat = JSON.parse(readCatalogue());
  main_web(JSON.stringify(album)); // runs forever
  const adopt = setInterval(() => {
    const canvas = document.querySelector('body > canvas');
    if (canvas) { $('#stage').append(canvas); clearInterval(adopt); }
  }, 50);

  $('#track').onchange = () => { ti = +$('#track').value; si = 0; sel = null; renderAll(); };
  $('#play').onclick = togglePlay;
  $('#undo').onclick = undo;
  $('#redo').onclick = redo;
  $('#save').onclick = save;
  $('#add-scene').onclick = addScene;
  $('#add-layer').onclick = addLayer;
  $('#add-hotspot').onclick = addHotspot;
  $('#add-band').onclick = addBand;
  document.addEventListener('keydown', keys);
  setupGrid();
  setupSpectrum();
  setupTimeline();
  renderAll();
  for (const t of album.tracks) decodeWave(t);
  requestAnimationFrame(loop);
}

function renderAll() {
  renderTopbar();
  renderLists();
  renderInspector();
  renderGrid();
  renderTimeline();
  renderSources();
}

// ---- state in / commands out ----

function push() {
  clearTimeout(pushTimer);
  pushTimer = setTimeout(() => send(JSON.stringify({ album })), 60);
}

// Every edit goes through here: remember for undo, push to the app, mark unsaved.
function commit(opts = {}) {
  if (!opts.silent) {
    history.push(JSON.stringify(album));
    if (history.length > 200) history.shift();
    future = [];
  }
  dirty = true;
  setStatus('unsaved');
  push();
  if (opts.lists !== false) renderLists();
  if (opts.inspector) renderInspector();
  if (opts.grid !== false) renderGrid();
  if (opts.timeline !== false) renderTimeline();
  if (opts.sources) renderSources();
}

function snapshotForUndo() { history.push(JSON.stringify(album)); future = []; }

function undo() {
  if (!history.length) return;
  future.push(JSON.stringify(album));
  album = JSON.parse(history.pop());
  ti = Math.min(ti, album.tracks.length - 1);
  sel = null;
  dirty = true; setStatus('unsaved'); push(); renderAll();
}

function redo() {
  if (!future.length) return;
  history.push(JSON.stringify(album));
  album = JSON.parse(future.pop());
  sel = null;
  dirty = true; setStatus('unsaved'); push(); renderAll();
}

async function save() {
  setStatus('saving…');
  try {
    const r = await fetch('album.json', { method: 'PUT', body: JSON.stringify(album, null, 1) + '\n' });
    if (!r.ok) throw new Error(await r.text());
    dirty = false;
    setStatus('saved ' + new Date().toLocaleTimeString());
  } catch (e) {
    setStatus('no dev server to save to — downloaded album.json instead');
    const a = el('a', { href: URL.createObjectURL(new Blob([JSON.stringify(album, null, 1) + '\n'], { type: 'application/json' })), download: 'album.json' });
    document.body.append(a); a.click(); a.remove();
  }
}

function setStatus(text) {
  const s = $('#status');
  s.textContent = text;
  s.classList.toggle('dirty', dirty);
}

function togglePlay() {
  if (st.playing) send(JSON.stringify({ pause: true }));
  else send(JSON.stringify({ play: true }));
}

function seek(bar) {
  const scene = sceneAt(track(), bar);
  if (!scene) { setStatus(`no scene covers bar ${bar}`); return; }
  follow = true;
  send(JSON.stringify({ seek: { scene: scene.id, bar } }));
}

function keys(e) {
  const typing = /INPUT|SELECT|TEXTAREA/.test(e.target.tagName);
  if ((e.ctrlKey || e.metaKey) && e.key === 's') { e.preventDefault(); save(); return; }
  if ((e.ctrlKey || e.metaKey) && e.key === 'z') { e.preventDefault(); undo(); return; }
  if ((e.ctrlKey || e.metaKey) && (e.key === 'y' || (e.key === 'Z'))) { e.preventDefault(); redo(); return; }
  if (typing) return;
  if (e.key === ' ') { e.preventDefault(); togglePlay(); }
  if (e.key === 'Delete' || e.key === 'Backspace') deleteSelected();
}

// ---- the loop: poll the app ----

let lastBar = -1;
function loop() {
  try { st = JSON.parse(readState() || '{}'); } catch { st = {}; }
  const t = track();
  $('#play').textContent = st.playing ? '❚❚' : '▶';
  $('#where').textContent = st.scene ? `${st.scene}  bar ${(st.bar ?? 0).toFixed(2)}${st.pending ? ' → ' + st.pending : ''}${st.loaded === false ? '  (loading)' : ''}` : '—';
  if (st.track && st.track === t.id) {
    $('#playhead').style.left = (GUTTER + st.bar * PPB) + 'px';
    $('#playhead').style.display = 'block';
  } else {
    $('#playhead').style.display = 'none';
  }
  // follow the music into whichever scene is playing, until a scene is picked by hand
  if (follow && st.track === t.id && !tl.drag) {
    const k = t.scenes.findIndex((s) => s.id === st.scene);
    if (k >= 0 && k !== si) { si = k; if (sel && sel.type !== 'band') sel = sel.type === 'scene' ? { type: 'scene', index: k } : null; renderLists(); renderTimeline(); renderInspector(); renderGrid(); }
  }
  const playing = $('#scene-list li.playing');
  const now = t.scenes.findIndex((s) => s.id === st.scene && st.track === t.id);
  if ((playing ? +playing.dataset.index : -1) !== now) {
    for (const li of document.querySelectorAll('#scene-list li')) li.classList.toggle('playing', +li.dataset.index === now);
  }
  for (const u of UNLOCKS) {
    const box = $(`#unlock-${u}`);
    if (box && document.activeElement !== box) { box.checked = (st.unlocks || []).includes(u); box.parentElement.classList.toggle('on', box.checked); }
  }
  drawSpectrum();
  updateMeters();
  requestAnimationFrame(loop);
}

// Built-in sources, the way data.rs computes them (noise and transition are per cell / per passage).
function builtins() {
  const whole = st.phase ?? 0;
  const half = (whole % 0.5) * 2, quarter = (half % 0.5) * 2;
  const hit = (p, f) => Math.pow(1 - clamp(p, 0, 1), f);
  const beat = hit(quarter, 2.5), bar = hit(whole, 1.5);
  const spec = lastSpectrum;
  const coarse = (a, b) => { let s = 0, n = 0; for (let k = a; k < b && k < spec.length; k++) { s += spec[k]; n++; } return n ? s / n / 255 : 0; };
  const per = spec.length / 16;
  return {
    beat, bar, pulse: beat * (0.7 + 0.3 * bar),
    noise: NaN, transition: 0,
    bass: coarse(0, 2 * per), mid: coarse(2 * per, 8 * per), treble: coarse(8 * per, 16 * per),
  };
}

function sourceLevel(name) {
  if (st.bands && name in st.bands) return st.bands[name];
  const b = builtins();
  return name in b ? b[name] : 0;
}

function shape(chain, level) {
  let t = clamp((level - chain.threshold) / Math.max(chain.width, 1e-6), 0, 1);
  t = t * t * (3 - 2 * t);
  if (chain.curve > 0 && chain.curve !== 1) t = Math.pow(t, chain.curve);
  return chain.invert ? 1 - t : t;
}

function updateMeters() {
  for (const m of document.querySelectorAll('.meter[data-source]')) {
    const v = sourceLevel(m.dataset.source);
    m.firstChild.style.width = (isNaN(v) ? 0 : clamp(v, 0, 1) * 100) + '%';
  }
  for (const m of document.querySelectorAll('.meter[data-chain]')) {
    const chain = chainRefs[m.dataset.chain];
    if (!chain) continue;
    const v = shape(chain, sourceLevel(chain.source));
    m.firstChild.style.width = (clamp(v, 0, 1) * 100) + '%';
    m.title = (chain.depth * v).toFixed(3);
  }
}

// ---- top bar ----

function renderTopbar() {
  const sel = $('#track');
  sel.replaceChildren(...album.tracks.map((t, k) => el('option', { value: k }, t.id)));
  sel.value = ti;
  $('#unlocks').replaceChildren(...UNLOCKS.map((u) =>
    el('label', {}, el('input', { type: 'checkbox', id: `unlock-${u}`, onchange: () => {
      const on = UNLOCKS.filter((v) => $(`#unlock-${v}`).checked);
      send(JSON.stringify({ unlocks: on }));
    } }), u)));
}

// ---- lists ----

function renderLists() {
  const t = track(), s = scene();
  const item = (type, index, label, bars, extra = '') => el('li', {
    class: (sel && sel.type === type && sel.index === index ? 'sel ' : '') + extra,
    'data-index': index,
    onclick: () => select(type, index),
  }, el('span', {}, label), el('span', { class: 'bars' }, `${bars[0]}–${bars[1]}`));
  $('#scene-list').replaceChildren(...t.scenes.map((s, k) => item('scene', k, s.id, s.bars, k === si ? 'here' : '')));
  $('#hotspot-title').textContent = s ? `hotspots of ${s.id}` : 'hotspots';
  $('#layer-title').textContent = s ? `layers of ${s.id}` : 'layers';
  $('#hotspot-list').replaceChildren(...(s?.hotspots ?? []).map((h, k) => item('hotspot', k, h.name || h.id, h.bars)));
  $('#layer-list').replaceChildren(...(s?.layers ?? []).map((l, k) => item('layer', k, `${l.name || l.id} · ${l.kind}`, l.bars, l.muted ? 'muted' : '')));
}

function select(type, index) {
  if (type === 'scene') { si = index; follow = false; }
  sel = { type, index };
  renderLists(); renderInspector(); renderGrid(); renderTimeline(); renderSources();
  sendHighlight();
}

function selected() {
  if (!sel) return null;
  const t = track(), s = scene();
  if (sel.type === 'scene') return t.scenes[sel.index];
  if (sel.type === 'layer') return s?.layers[sel.index];
  if (sel.type === 'hotspot') return s?.hotspots[sel.index];
  if (sel.type === 'band') return album.bands[sel.index];
  return null;
}

function listOf(type) {
  const t = track(), s = scene();
  return type === 'scene' ? t.scenes : type === 'layer' ? s.layers : type === 'hotspot' ? s.hotspots : album.bands;
}

function deleteSelected() {
  if (!sel) return;
  const list = listOf(sel.type);
  snapshotForUndo();
  list.splice(sel.index, 1);
  if (sel.type === 'scene') si = Math.max(0, Math.min(si, track().scenes.length - 1));
  sel = null;
  commit({ silent: true, inspector: true, sources: true });
}

function duplicateSelected() {
  if (!sel || sel.type === 'band') return;
  const list = listOf(sel.type);
  const copy = JSON.parse(JSON.stringify(list[sel.index]));
  if (sel.type === 'scene') { copy.id = copy.id + '-copy'; const len = copy.bars[1] - copy.bars[0]; const old = [...copy.bars]; copy.bars = [copy.bars[1], copy.bars[1] + len]; fitScene(copy, old); }
  else { copy.id = uid(sel.type[0]); copy.name = (copy.name || copy.id) + ' copy'; }
  snapshotForUndo();
  list.splice(sel.index + 1, 0, copy);
  sel = { type: sel.type, index: sel.index + 1 };
  if (sel.type === 'scene') si = sel.index;
  commit({ silent: true, inspector: true });
}

// Move a layer or hotspot to another scene of the track (by id).
function moveSelectedTo(sceneId) {
  const t = track();
  const k = t.scenes.findIndex((s) => s.id === sceneId);
  if (k < 0 || k === si || !sel || (sel.type !== 'layer' && sel.type !== 'hotspot')) return;
  snapshotForUndo();
  const list = listOf(sel.type);
  const [item] = list.splice(sel.index, 1);
  const to = t.scenes[k];
  item.bars = [...to.bars];
  to[sel.type + 's'].push(item);
  si = k;
  sel = { type: sel.type, index: to[sel.type + 's'].length - 1 };
  commit({ silent: true, inspector: true });
}

// ---- adding ----

function defaultBars() {
  const s = scene();
  return s ? [...s.bars] : [0, 16];
}

function defaultParams(kind) {
  const out = {};
  for (const [name, spec] of Object.entries(cat.kinds[kind].params)) {
    out[name] = spec.type === 'param' ? { base: spec.default, chains: [] } : spec.default;
  }
  return out;
}

function addScene() {
  const t = track();
  const end = Math.max(0, ...t.scenes.map((s) => s.bars[1]));
  snapshotForUndo();
  t.scenes.push({ id: `${t.id}/${t.scenes.length}`, bars: [end, end + 16], layers: [], hotspots: [] });
  si = t.scenes.length - 1;
  sel = { type: 'scene', index: si };
  commit({ silent: true, inspector: true });
}

function addLayer() {
  const s = scene();
  if (!s) { setStatus('add a scene first'); return; }
  snapshotForUndo();
  s.layers ??= [];
  s.layers.push({ id: uid('l'), name: 'layer', bars: defaultBars(), region: {}, kind: 'marks', params: defaultParams('marks') });
  sel = { type: 'layer', index: s.layers.length - 1 };
  commit({ silent: true, inspector: true });
}

function addHotspot() {
  const s = scene();
  if (!s) { setStatus('add a scene first'); return; }
  snapshotForUndo();
  s.hotspots ??= [];
  s.hotspots.push({ id: uid('h'), name: 'hotspot', bars: defaultBars(), rect: [8, 8, 12, 12], needs: [], on: 'tap', then: [] });
  sel = { type: 'hotspot', index: s.hotspots.length - 1 };
  commit({ silent: true, inspector: true });
}

function addBand() {
  snapshotForUndo();
  album.bands.push({ id: uid('b'), name: 'band', lo: 100, hi: 400, attack: 0.01, release: 0.15, gain: 1 });
  sel = { type: 'band', index: album.bands.length - 1 };
  commit({ silent: true, inspector: true, sources: true });
}

// ---- inspector ----

const chainRefs = {}; // meter id → chain object, for the live meters

function numberInput(obj, key, opts = {}) {
  return el('input', {
    type: 'number', value: obj[key] ?? 0, step: opts.step ?? 0.1, min: opts.min, max: opts.max,
    onchange: (e) => { snapshotForUndo(); obj[key] = +e.target.value; commit({ silent: true, ...opts.commit }); },
  });
}

function textInput(obj, key, opts = {}) {
  return el('input', {
    type: 'text', value: obj[key] ?? '',
    onchange: (e) => { snapshotForUndo(); obj[key] = e.target.value; commit({ silent: true, ...opts.commit }); },
  });
}

function selectInput(obj, key, options, opts = {}) {
  const s = el('select', { onchange: (e) => { snapshotForUndo(); obj[key] = e.target.value; commit({ silent: true, ...opts.commit }); if (opts.then) opts.then(); } },
    ...options.map((o) => el('option', { value: typeof o === 'string' ? o : o.value }, typeof o === 'string' ? o : o.label)));
  s.value = obj[key] ?? '';
  return s;
}

function checkInput(obj, key, label, opts = {}) {
  return el('label', {}, el('input', { type: 'checkbox', ...(obj[key] ? { checked: '' } : {}),
    onchange: (e) => { snapshotForUndo(); obj[key] = e.target.checked; commit({ silent: true, ...opts.commit }); if (opts.then) opts.then(); } }), label);
}

function barsRow(obj, isScene) {
  const s = scene();
  const fix = () => {
    if (isScene) return;
    obj.bars[0] = clamp(obj.bars[0], s.bars[0], s.bars[1] - 1);
    obj.bars[1] = clamp(obj.bars[1], obj.bars[0] + 1, s.bars[1]);
  };
  const input = (k) => el('input', { type: 'number', value: obj.bars[k], step: 1, min: 0, onchange: (e) => {
    snapshotForUndo();
    const old = [...obj.bars];
    obj.bars[k] = +e.target.value;
    if (isScene) fitScene(obj, old); else fix();
    commit({ silent: true, inspector: true });
  } });
  return el('div', { class: 'row' }, el('span', { class: 'name' }, 'bars'), input(0), '–', input(1),
    el('span', { class: 'doc' }, isScene ? 'of the track, end exclusive; its layers and hotspots move with it' : `within ${s.bars[0]}–${s.bars[1]}, the scene`));
}

function row(name, ...content) {
  return el('div', { class: 'row' }, el('span', { class: 'name' }, name), ...content);
}

function renderInspector() {
  const box = $('#inspector');
  const item = selected();
  for (const k in chainRefs) delete chainRefs[k];
  if (!item) { box.replaceChildren(el('div', { class: 'hint' }, 'select a scene, a layer, a hotspot or a band')); return; }
  const t = track();
  const head = (title, kind) => el('h2', {}, title, el('span', { class: 'kind' }, kind), el('span', { class: 'spacer' }),
    sel.type !== 'band' ? el('button', { class: 'small', onclick: duplicateSelected }, 'duplicate') : null,
    el('button', { class: 'small danger', onclick: deleteSelected }, 'delete'));

  if (sel.type === 'scene') {
    box.replaceChildren(
      head(item.id, 'scene'),
      row('id', textInput(item, 'id', { commit: { inspector: true } }), el('span', { class: 'doc' }, 'what doors point at; renaming does not follow them')),
      barsRow(item, true),
      row('', el('button', { onclick: () => seek(item.bars[0]) }, 'play from here')),
      row('layers', el('span', { class: 'doc' }, (item.layers || []).map((l) => l.name || l.id).join(', ') || 'none')),
      row('hotspots', el('span', { class: 'doc' }, (item.hotspots || []).map((h) => h.name || h.id).join(', ') || 'none')),
    );
  } else if (sel.type === 'layer') {
    box.replaceChildren(head(item.name || item.id, item.kind), ...layerInspector(item));
  } else if (sel.type === 'hotspot') {
    box.replaceChildren(head(item.name || item.id, 'hotspot'), ...hotspotInspector(item));
  } else if (sel.type === 'band') {
    box.replaceChildren(head(item.name || item.id, 'band'), ...bandInspector(item));
  }
}

function moveRow() {
  const t = track();
  return row('in scene', el('select', { onchange: (e) => moveSelectedTo(e.target.value) },
    ...t.scenes.map((s, k) => el('option', { value: s.id, ...(k === si ? { selected: '' } : {}) }, s.id))),
    el('span', { class: 'doc' }, 'pick another to move it there'));
}

function layerInspector(l) {
  const s = scene();
  const kinds = Object.keys(cat.kinds);
  const spec = cat.kinds[l.kind] || cat.kinds.marks;
  l.params ??= {};
  const out = [
    row('name', textInput(l, 'name')),
    row('kind', selectInput(l, 'kind', kinds, { commit: { inspector: true }, then: () => { l.params = defaultParams(l.kind); commit({ silent: true, inspector: true }); } }),
      el('span', { class: 'doc' }, spec.doc)),
    barsRow(l),
    moveRow(),
    row('', checkInput(l, 'muted', 'muted')),
    el('fieldset', {}, el('legend', {}, 'params'), ...Object.entries(spec.params).map(([name, p]) => paramRow(l.params, name, p))),
  ];
  if (!spec.modifier) {
    const glow = l.glow ?? 'region';
    out.push(row('glow', el('select', { onchange: (e) => { snapshotForUndo(); if (e.target.value === 'region') delete l.glow; else l.glow = e.target.value; commit({ silent: true }); } },
      ...['region', 'field', 'inverse', 'bounded', 'off'].map((o) => el('option', { value: o, ...(o === glow ? { selected: '' } : {}) }, o))),
      el('span', { class: 'doc' }, 'region: whatever glow layer covers the cell decides (field on the pulse where none); the rest force a mode')));
  }
  // gates
  const gates = el('fieldset', {}, el('legend', {}, 'showing when'));
  const when = l.when;
  gates.append(row('hotspot', el('select', { onchange: (e) => {
      snapshotForUndo();
      if (!e.target.value) delete l.when; else l.when = { hotspot: e.target.value, visible: l.when?.visible ?? true };
      commit({ silent: true, inspector: true });
    } }, el('option', { value: '' }, '—'), ...(s.hotspots || []).map((h) => el('option', { value: h.id, ...(when?.hotspot === h.id ? { selected: '' } : {}) }, h.name || h.id))),
    when ? el('select', { onchange: (e) => { snapshotForUndo(); l.when.visible = e.target.value === 'visible'; commit({ silent: true }); } },
      el('option', { value: 'visible', ...(when.visible ? { selected: '' } : {}) }, 'is showing'),
      el('option', { value: 'hidden', ...(!when.visible ? { selected: '' } : {}) }, 'is not showing')) : null,
    el('span', { class: 'doc' }, 'hints: a layer tied to a hotspot the player can see')));
  gates.append(row('fade', el('select', { onchange: (e) => {
      snapshotForUndo();
      if (!e.target.value) delete l.fade; else l.fade = { unlock: e.target.value, to: l.fade?.to ?? 1 };
      commit({ silent: true, inspector: true });
    } }, el('option', { value: '' }, '—'), ...UNLOCKS.map((u) => el('option', { value: u, ...(l.fade?.unlock === u ? { selected: '' } : {}) }, u))),
    l.fade ? el('select', { onchange: (e) => { snapshotForUndo(); l.fade.to = +e.target.value; commit({ silent: true }); } },
      el('option', { value: 1, ...(l.fade.to >= 0.5 ? { selected: '' } : {}) }, 'fades in as it is unlocked'),
      el('option', { value: 0, ...(l.fade.to < 0.5 ? { selected: '' } : {}) }, 'fades out as it is unlocked')) : null));
  l.needs ??= [];
  gates.append(row('needs', ...UNLOCKS.map((u) => el('label', {}, el('input', { type: 'checkbox', ...(l.needs.includes(u) ? { checked: '' } : {}), onchange: (e) => {
    snapshotForUndo();
    l.needs = UNLOCKS.filter((v) => (v === u ? e.target.checked : l.needs.includes(v)));
    commit({ silent: true });
  } }), u)), el('span', { class: 'doc' }, 'black without these')));
  out.push(gates);
  return out;
}

function paramRow(params, name, spec) {
  // Missing in the file means the engine's default: show that, not 0.
  if (params[name] === undefined) params[name] = spec.type === 'param' ? { base: spec.default, chains: [] } : spec.default;
  if (spec.type === 'param') {
    if (typeof params[name] !== 'object' || params[name] === null) params[name] = { base: typeof params[name] === 'number' ? params[name] : spec.default, chains: [] };
    const p = params[name];
    p.chains ??= [];
    const chainsBox = el('div', { class: 'chains' });
    const renderChains = () => {
      chainsBox.replaceChildren(...p.chains.map((c, k) => chainRow(p, c, k, renderChains)),
        el('button', { class: 'link', onclick: () => { snapshotForUndo(); p.chains.push({ source: album.bands[0]?.id ?? 'beat', depth: 1, threshold: 0, width: 1, curve: 1, invert: false, on: true }); commit({ silent: true }); renderChains(); } }, '+ sidechain'));
    };
    renderChains();
    chainsBox.hidden = p.chains.length === 0;
    return el('div', {}, row(name, numberInput(p, 'base', { step: spec.step ?? 0.1 }),
      el('button', { class: 'link', title: 'sidechain this parameter from a source', onclick: () => { chainsBox.hidden = !chainsBox.hidden; } }, p.chains.length ? `${p.chains.length} sidechain${p.chains.length > 1 ? 's' : ''}` : 'sidechain'),
      spec.doc ? el('span', { class: 'doc' }, spec.doc) : null), chainsBox);
  }
  if (spec.type === 'number') return row(name, numberInput(params, name, { step: spec.step ?? 0.1 }), spec.doc ? el('span', { class: 'doc' }, spec.doc) : null);
  if (spec.type === 'bool') return row(name, checkInput(params, name, spec.doc || ''));
  if (spec.type === 'select') return row(name, selectInput(params, name, spec.options), spec.doc ? el('span', { class: 'doc' }, spec.doc) : null);
  if (spec.type === 'angle') {
    const named = ['along', 'across', 'noise', 'checker', 'around'];
    const isNum = typeof params[name] === 'number';
    const sel = el('select', { onchange: (e) => { snapshotForUndo(); params[name] = e.target.value === 'degrees' ? 0 : e.target.value; commit({ silent: true, inspector: true }); } },
      ...named.map((o) => el('option', { value: o, ...(params[name] === o ? { selected: '' } : {}) }, o)), el('option', { value: 'degrees', ...(isNum ? { selected: '' } : {}) }, 'degrees'));
    return row(name, sel, isNum ? numberInput(params, name, { step: 15 }) : null, el('span', { class: 'doc' }, spec.doc));
  }
  return row(name, el('span', { class: 'doc' }, 'unknown param type ' + spec.type));
}

function chainRow(p, c, k, rerender) {
  const id = uid('m');
  chainRefs[id] = c;
  const sources = [...album.bands.map((b) => ({ value: b.id, label: b.name || b.id })), ...cat.sources.map((s) => ({ value: s, label: s }))];
  const num = (key, step) => el('input', { type: 'number', value: c[key], step, onchange: (e) => { snapshotForUndo(); c[key] = +e.target.value; commit({ silent: true }); } });
  return el('div', { class: 'chain' + (c.on ? '' : ' off') },
    el('label', {}, el('input', { type: 'checkbox', ...(c.on ? { checked: '' } : {}), title: 'on', onchange: (e) => { snapshotForUndo(); c.on = e.target.checked; commit({ silent: true }); rerender(); } })),
    selectInput(c, 'source', sources),
    el('div', { class: 'meter', 'data-chain': id, title: 'what it adds right now' }, el('i')),
    el('span', { class: 'lbl' }, 'depth'), num('depth', 0.1),
    el('span', { class: 'lbl' }, 'threshold'), num('threshold', 0.05),
    el('span', { class: 'lbl' }, 'width'), num('width', 0.05),
    el('span', { class: 'lbl' }, 'curve'), num('curve', 0.1),
    el('label', {}, el('input', { type: 'checkbox', ...(c.invert ? { checked: '' } : {}), onchange: (e) => { snapshotForUndo(); c.invert = e.target.checked; commit({ silent: true }); } }), 'invert'),
    el('button', { class: 'link danger', onclick: () => { snapshotForUndo(); p.chains.splice(k, 1); commit({ silent: true }); rerender(); } }, '×'));
}

function hotspotInspector(h) {
  h.needs ??= []; h.then ??= [];
  const on = typeof h.on === 'string' ? h.on : 'drag';
  const onRow = row('on', el('select', { onchange: (e) => {
      snapshotForUndo();
      h.on = e.target.value === 'drag' ? { drag: [...h.rect] } : e.target.value;
      commit({ silent: true, inspector: true });
    } }, ...['tap', 'outline', 'drag'].map((o) => el('option', { value: o, ...(o === on ? { selected: '' } : {}) }, o))),
    el('span', { class: 'doc' }, on === 'tap' ? 'press and release inside' : on === 'outline' ? 'draw its perimeter in one stroke' : 'press inside, release in the target'));
  const dragRow = on === 'drag' ? row('drag to', ...[0, 1, 2, 3].map((k) => numberInput(h.on.drag, k, { step: 1, min: 0, max: 20 })), el('span', { class: 'doc' }, 'i0 j0 i1 j1')) : null;
  const actions = el('div', { class: 'actions' });
  const renderActions = () => {
    actions.replaceChildren(...h.then.map((a, k) => {
      const kind = Object.keys(a)[0];
      const kindSel = el('select', { onchange: (e) => {
        snapshotForUndo();
        const v = e.target.value;
        h.then[k] = v === 'go' ? { go: allSceneIds()[0] || '' } : v === 'unlock' ? { unlock: 'glow' } : { via: [allSceneIds()[0] || '', [...h.bars]] };
        commit({ silent: true }); renderActions();
      } }, ...['go', 'unlock', 'via'].map((o) => el('option', { value: o, ...(o === kind ? { selected: '' } : {}) }, o)));
      let body;
      if (kind === 'go') body = selectInput(a, 'go', allSceneIds());
      else if (kind === 'unlock') body = selectInput(a, 'unlock', UNLOCKS);
      else body = el('span', {}, selectInput(a.via, 0, allSceneIds()), ' after bars ', numberInput(a.via[1], 0, { step: 1 }), '–', numberInput(a.via[1], 1, { step: 1 }), el('span', { class: 'doc' }, ' of this track, played once'));
      return el('div', { class: 'action' }, kindSel, body, el('button', { class: 'link danger', onclick: () => { snapshotForUndo(); h.then.splice(k, 1); commit({ silent: true }); renderActions(); } }, '×'));
    }), el('button', { class: 'link', onclick: () => { snapshotForUndo(); h.then.push({ go: allSceneIds()[0] || '' }); commit({ silent: true }); renderActions(); } }, '+ action'));
  };
  renderActions();
  return [
    row('name', textInput(h, 'name')),
    row('id', el('span', { class: 'mono' }, h.id), el('span', { class: 'doc' }, 'layers refer to it by this')),
    barsRow(h),
    moveRow(),
    row('cells', ...[0, 1, 2, 3].map((k) => numberInput(h.rect, k, { step: 1, min: 0, max: 20 })), el('span', { class: 'doc' }, 'i0 j0 i1 j1 — or drag on the grid')),
    onRow, dragRow,
    row('needs', ...UNLOCKS.map((u) => el('label', {}, el('input', { type: 'checkbox', ...(h.needs.includes(u) ? { checked: '' } : {}), onchange: (e) => {
      snapshotForUndo(); h.needs = UNLOCKS.filter((v) => (v === u ? e.target.checked : h.needs.includes(v))); commit({ silent: true });
    } }), u)), el('span', { class: 'doc' }, 'invisible and inert until the player has these')),
    el('fieldset', {}, el('legend', {}, 'then'), actions,
      el('div', { class: 'doc' }, 'an unlock never moves you; a spent unlock hotspot disappears, so a second hotspot on the same cells that needs it can be the door')),
  ];
}

function bandInspector(b) {
  return [
    row('name', textInput(b, 'name', { commit: { sources: true } })),
    row('id', el('span', { class: 'mono' }, b.id)),
    row('range', numberInput(b, 'lo', { step: 10, min: 0, commit: { sources: true } }), '–', numberInput(b, 'hi', { step: 10, min: 0, commit: { sources: true } }), el('span', { class: 'doc' }, 'Hz; or drag its edges on the spectrum')),
    row('attack', numberInput(b, 'attack', { step: 0.005, min: 0 }), el('span', { class: 'doc' }, 'seconds to rise')),
    row('release', numberInput(b, 'release', { step: 0.01, min: 0 }), el('span', { class: 'doc' }, 'seconds to fall')),
    row('gain', numberInput(b, 'gain', { step: 0.1, min: 0 }), el('span', { class: 'doc' }, 'level multiplier (high bands are quiet)')),
    row('level', el('div', { class: 'meter', 'data-source': b.id }, el('i'))),
  ];
}

// ---- the grid: regions ----

const grid = { drag: null };

function regionOf() {
  if (!sel) return null;
  const item = selected();
  if (sel.type === 'layer') return item.region ??= {};
  if (sel.type === 'hotspot') return { rect: item.rect };
  return null;
}

// Mirrors data::Region::contains.
function inRegion(r, i, j) {
  const [i0, j0, i1, j1] = r.rect ?? [0, 0, W, H];
  if (!(i0 <= i && i < i1 && j0 <= j && j < j1)) return false;
  if (r.perimeter && !(i === i0 || i + 1 === i1 || j === j0 || j + 1 === j1)) return false;
  if (r.map != null && mapAt(r.map, i, j) !== '#') return false;
  if (r.rows && r.rows[0] > 1 && j % r.rows[0] !== r.rows[1] % r.rows[0]) return false;
  if (r.cols && r.cols[0] > 1 && i % r.cols[0] !== r.cols[1] % r.cols[0]) return false;
  if (r.checker != null && (i + j) % 2 !== r.checker % 2) return false;
  return true;
}

function mapAt(map, i, j) {
  const flat = map.replace(/\s+/g, '');
  return flat[(H - 1 - j) * W + i] ?? '.';
}

function mapSet(map, i, j, on) {
  const rows = mapRows(map);
  const r = H - 1 - j;
  rows[r] = rows[r].slice(0, i) + (on ? '#' : '.') + rows[r].slice(i + 1);
  return rows.join('\n');
}

function mapRows(map) {
  const flat = (map || '').replace(/\s+/g, '').padEnd(W * H, '.');
  return Array.from({ length: H }, (_, r) => flat.slice(r * W, (r + 1) * W));
}

function regionCells(r) {
  const cells = [];
  for (let i = 0; i < W; i++) for (let j = 0; j < H; j++) if (inRegion(r, i, j)) cells.push([i, j]);
  return cells;
}

function sendHighlight() {
  const r = regionOf();
  send(JSON.stringify({ highlight: r ? regionCells(r) : [] }));
}

function setupGrid() {
  const c = $('#grid');
  const cellAt = (e) => {
    const rect = c.getBoundingClientRect();
    const i = clamp(Math.floor((e.clientX - rect.left) / CELL), 0, W - 1);
    const j = clamp(H - 1 - Math.floor((e.clientY - rect.top) / CELL), 0, H - 1);
    return [i, j];
  };
  c.onmousedown = (e) => {
    const r = regionOf();
    if (!r) return;
    const [i, j] = cellAt(e);
    snapshotForUndo();
    if (sel.type === 'layer' && r.map != null) {
      grid.drag = { paint: mapAt(r.map, i, j) !== '#' };
      r.map = mapSet(r.map, i, j, grid.drag.paint);
    } else {
      grid.drag = { from: [i, j] };
      setRect(r, i, j, i + 1, j + 1);
    }
    commit({ silent: true, timeline: false, lists: false });
  };
  c.onmousemove = (e) => {
    if (!grid.drag) return;
    const r = regionOf();
    const [i, j] = cellAt(e);
    if (grid.drag.paint != null) {
      if (mapAt(r.map, i, j) === '#' !== grid.drag.paint) return;
      r.map = mapSet(r.map, i, j, grid.drag.paint);
    } else {
      const [a, b] = grid.drag.from;
      setRect(r, Math.min(a, i), Math.min(b, j), Math.max(a, i) + 1, Math.max(b, j) + 1);
    }
    commit({ silent: true, timeline: false, lists: false });
  };
  window.addEventListener('mouseup', () => { if (grid.drag) { grid.drag = null; renderInspector(); } });
}

function setRect(r, i0, j0, i1, j1) {
  if (sel.type === 'hotspot') { const h = selected(); h.rect[0] = i0; h.rect[1] = j0; h.rect[2] = i1; h.rect[3] = j1; }
  else r.rect = [i0, j0, i1, j1];
}

function renderGrid() {
  const c = $('#grid'), g = c.getContext('2d');
  g.fillStyle = '#000'; g.fillRect(0, 0, c.width, c.height);
  const r = regionOf();
  for (let i = 0; i < W; i++) for (let j = 0; j < H; j++) {
    const x = i * CELL, y = (H - 1 - j) * CELL;
    const inside = r && inRegion(r, i, j);
    const inRect = r && r.rect && r.rect[0] <= i && i < r.rect[2] && r.rect[1] <= j && j < r.rect[3];
    g.fillStyle = inside ? (sel.type === 'hotspot' ? '#b08040' : '#4a86c8') : inRect ? '#1d2a3a' : '#15151a';
    g.fillRect(x + 1, y + 1, CELL - 2, CELL - 2);
    if (!inside) { g.fillStyle = '#3a3a44'; g.fillRect(x + CELL / 2, y + CELL / 2, 1, 1); }
  }
  sendHighlight();
  // tools
  const tools = $('#region-tools');
  if (!r || sel.type !== 'layer') { tools.replaceChildren(r ? el('span', {}, 'drag to set the hotspot\'s cells') : el('span', {}, 'select a layer or hotspot to edit its region')); return; }
  const l = selected();
  const go = () => commit({ silent: true, lists: false, timeline: false });
  const btn = (label, on, fn) => el('button', { class: 'small' + (on ? ' on' : ''), onclick: () => { snapshotForUndo(); fn(); go(); } }, label);
  tools.replaceChildren(
    btn('whole', !r.rect && r.map == null, () => { delete r.rect; delete r.map; }),
    btn('rect', !!r.rect && r.map == null, () => { delete r.map; r.rect ??= [5, 5, 15, 15]; }),
    btn('map', r.map != null, () => { if (r.map == null) { const rows = mapRows(''); r.map = rows.map((row, k) => Array.from(row, (_, i) => inRegion({ rect: r.rect }, i, H - 1 - k) ? '#' : '.').join('')).join('\n'); delete r.rect; } }),
    el('label', {}, el('input', { type: 'checkbox', ...(r.perimeter ? { checked: '' } : {}), onchange: (e) => { snapshotForUndo(); if (e.target.checked) r.perimeter = true; else delete r.perimeter; go(); } }), 'perimeter'),
    el('span', {}, 'rows every ', stepInput(r, 'rows'), ' cols every ', stepInput(r, 'cols')),
    el('span', {}, 'checker ', el('select', { onchange: (e) => { snapshotForUndo(); if (e.target.value === '') delete r.checker; else r.checker = +e.target.value; go(); } },
      ...[['', 'both'], ['0', 'even'], ['1', 'odd']].map(([v, t]) => el('option', { value: v, ...(String(r.checker ?? '') === v ? { selected: '' } : {}) }, t)))),
    el('span', { class: 'doc' }, r.map != null ? 'click/drag paints cells' : 'drag draws the rectangle'),
  );
}

function stepInput(r, key) {
  const v = r[key] ?? [1, 0];
  return el('span', {}, el('input', { type: 'number', value: v[0], step: 1, min: 1, onchange: (e) => { snapshotForUndo(); const n = +e.target.value; if (n <= 1) delete r[key]; else r[key] = [n, (r[key] ?? [1, 0])[1]]; commit({ silent: true, lists: false, timeline: false }); } }),
    ' from ', el('input', { type: 'number', value: v[1], step: 1, min: 0, onchange: (e) => { snapshotForUndo(); if (r[key]) { r[key][1] = +e.target.value; commit({ silent: true, lists: false, timeline: false }); } } }));
}

// ---- sources: the spectrum and the bands ----

let lastSpectrum = new Uint8Array(0);
const spec = { drag: null };
const FMIN = 20, FMAX = 20000;
const fx = (hz, w) => (Math.log(clamp(hz, FMIN, FMAX) / FMIN) / Math.log(FMAX / FMIN)) * w;
const xf = (x, w) => FMIN * Math.pow(FMAX / FMIN, clamp(x / w, 0, 1));

function setupSpectrum() {
  const c = $('#spectrum');
  const w = () => c.getBoundingClientRect().width;
  const px = (e) => e.clientX - c.getBoundingClientRect().left;
  c.onmousedown = (e) => {
    const x = px(e), width = w();
    let best = null;
    album.bands.forEach((b, k) => {
      for (const edge of ['lo', 'hi']) {
        const d = Math.abs(fx(b[edge], width) - x);
        if (d < 6 && (!best || d < best.d)) best = { k, edge, d };
      }
    });
    if (!best) {
      // click inside a band selects it
      const k = album.bands.findIndex((b) => fx(b.lo, width) <= x && x <= fx(b.hi, width));
      if (k >= 0) select('band', k);
      return;
    }
    snapshotForUndo();
    spec.drag = best;
    select('band', best.k);
  };
  c.onmousemove = (e) => {
    if (!spec.drag) return;
    const b = album.bands[spec.drag.k];
    const hz = Math.round(xf(px(e), w()));
    if (spec.drag.edge === 'lo') b.lo = Math.min(hz, b.hi - 1); else b.hi = Math.max(hz, b.lo + 1);
    commit({ silent: true, lists: false, grid: false, timeline: false });
  };
  window.addEventListener('mouseup', () => { if (spec.drag) { spec.drag = null; renderInspector(); renderSources(); } });
}

function drawSpectrum() {
  const c = $('#spectrum'), g = c.getContext('2d');
  const width = c.width, height = c.height;
  lastSpectrum = readSpectrum();
  g.fillStyle = '#000'; g.fillRect(0, 0, width, height);
  const binHz = st.binHz || 21.5;
  // bars: one per pixel column, the max of the bins under it
  g.fillStyle = '#5a5a66';
  let prevBin = 0;
  for (let x = 0; x < width; x++) {
    const hz = xf(x + 1, width);
    const bin = Math.min(lastSpectrum.length, Math.max(prevBin + 1, Math.floor(hz / binHz)));
    let m = 0;
    for (let k = prevBin; k < bin; k++) m = Math.max(m, lastSpectrum[k] || 0);
    prevBin = Math.max(prevBin, bin - 1);
    const h = (m / 255) * height;
    g.fillRect(x, height - h, 1, h);
  }
  // bands
  album.bands.forEach((b, k) => {
    const x0 = fx(b.lo, width), x1 = fx(b.hi, width);
    const color = BAND_COLORS[k % BAND_COLORS.length];
    g.fillStyle = color + (sel && sel.type === 'band' && sel.index === k ? '55' : '2a');
    g.fillRect(x0, 0, x1 - x0, height);
    g.fillStyle = color;
    g.fillRect(x0, 0, 1, height); g.fillRect(x1 - 1, 0, 1, height);
    const level = st.bands?.[b.id] ?? 0;
    g.fillRect(x0, height - level * height, x1 - x0, 2);
    g.font = '10px system-ui'; g.fillText(b.name || b.id, x0 + 3, 11);
  });
  // ticks
  g.fillStyle = '#8a8a94'; g.font = '9px system-ui';
  for (const hz of [50, 100, 200, 500, 1000, 2000, 5000, 10000]) {
    const x = fx(hz, width);
    g.fillRect(x, height - 4, 1, 4);
    g.fillText(hz >= 1000 ? hz / 1000 + 'k' : hz, x + 2, height - 2);
  }
}

function renderSources() {
  $('#bands').replaceChildren(...album.bands.map((b, k) => el('div', { class: 'band' + (sel && sel.type === 'band' && sel.index === k ? ' sel' : ''), onclick: (e) => { if (e.target.tagName !== 'INPUT') select('band', k); } },
    el('span', { class: 'swatch', style: `background:${BAND_COLORS[k % BAND_COLORS.length]}` }),
    textInput(b, 'name', { commit: { sources: true } }),
    numberInput(b, 'lo', { step: 10, min: 0 }), '–', numberInput(b, 'hi', { step: 10, min: 0 }), el('span', { class: 'dim' }, 'Hz'),
    el('span', { class: 'dim' }, 'att'), numberInput(b, 'attack', { step: 0.005, min: 0 }),
    el('span', { class: 'dim' }, 'rel'), numberInput(b, 'release', { step: 0.01, min: 0 }),
    el('span', { class: 'dim' }, 'gain'), numberInput(b, 'gain', { step: 0.1, min: 0 }),
    el('div', { class: 'meter', 'data-source': b.id }, el('i')))));
  $('#builtins').replaceChildren(...cat.sources.map((s) => el('div', { class: 'builtin', title: cat.sourceDocs?.[s] || '' },
    el('span', { class: 'name' }, s), el('div', { class: 'meter', 'data-source': s }, el('i')))));
}

// ---- the timeline ----

const tl = { drag: null };

function setupTimeline() {
  $('#ruler').onclick = (e) => {
    const x = e.clientX - $('#lanes').getBoundingClientRect().left - GUTTER;
    if (x >= 0) seek(Math.floor(x / PPB));
  };
  $('#wave').onclick = $('#ruler').onclick;
  $('#wave').append(el('span', { class: 'lane-label' }, ''));
  window.addEventListener('mousemove', (e) => {
    if (!tl.drag) return;
    const d = tl.drag;
    const bar = Math.round((e.clientX - d.x0) / PPB);
    const bars = d.obj.bars;
    const [lo, hi] = d.type === 'scene' ? [0, Infinity] : scene().bars;
    if (d.mode === 'move') {
      const len = d.bars[1] - d.bars[0];
      bars[0] = clamp(d.bars[0] + bar, lo, Math.max(lo, hi - len)); bars[1] = bars[0] + len;
    } else if (d.mode === 'l') {
      bars[0] = clamp(d.bars[0] + bar, lo, d.bars[1] - 1);
    } else {
      bars[1] = clamp(d.bars[1] + bar, d.bars[0] + 1, hi);
    }
    if (d.type === 'scene') fitScene(d.obj, d.last ?? d.bars);
    d.last = [...bars];
    if (d.hot) { d.el.style.left = GUTTER + bars[0] * PPB + 'px'; d.el.style.width = (bars[1] - bars[0]) * PPB - 2 + 'px'; }
    push();
  });
  window.addEventListener('mouseup', () => {
    if (!tl.drag) return;
    tl.drag = null;
    commit({ silent: true, inspector: true });
  });
}

function block(type, obj, index, label, extra = '') {
  const b = el('div', {
    class: `block ${type} ${extra}` + (sel && sel.type === type && sel.index === index ? ' sel' : ''),
    style: `left:${GUTTER + obj.bars[0] * PPB}px;width:${(obj.bars[1] - obj.bars[0]) * PPB - 2}px`,
    title: `${label} ${obj.bars[0]}–${obj.bars[1]}`,
    onmousedown: (e) => {
      select(type, index);
      const mode = e.target.classList.contains('handle') ? (e.target.classList.contains('l') ? 'l' : 'r') : 'move';
      snapshotForUndo();
      tl.drag = { obj, type, mode, x0: e.clientX, bars: [...obj.bars], el: b, hot: true };
      e.preventDefault();
    },
  }, el('div', { class: 'handle l' }), label, el('div', { class: 'handle r' }));
  return b;
}

// Lay blocks out in sub-rows so overlapping ranges don't hide each other.
function packed(type, items, label) {
  const rows = [];
  const placed = items.map((obj, index) => {
    let r = rows.findIndex((end) => end <= obj.bars[0]);
    if (r < 0) { r = rows.length; rows.push(0); }
    rows[r] = obj.bars[1];
    return { obj, index, r };
  });
  const lane = el('div', { class: 'lane', style: `height:${Math.max(1, rows.length) * 22}px` }, el('span', { class: 'lane-label' }, label));
  for (const { obj, index, r } of placed) {
    const b = block(type, obj, index, obj.id && type === 'scene' ? obj.id : obj.name || obj.id);
    b.style.top = 2 + r * 22 + 'px';
    lane.append(b);
  }
  return lane;
}

function renderTimeline() {
  const t = track();
  const bars = trackBars(t);
  const width = GUTTER + bars * PPB + 40;
  $('#lanes').style.width = width + 'px';
  const ruler = $('#ruler');
  ruler.replaceChildren(el('span', { class: 'lane-label' }, 'bars'), ...Array.from({ length: bars + 1 }, (_, k) => el('div', { class: 'tick' + (k % 4 === 0 ? ' four' : ''), style: `left:${GUTTER + k * PPB}px` }, k % 4 === 0 ? k : '')));
  const s = scene();
  $('#scene-lane').replaceWith(Object.assign(packed('scene', t.scenes, 'scenes'), { id: 'scene-lane' }));
  $('#hotspot-lane').replaceWith(Object.assign(packed('hotspot', s?.hotspots ?? [], 'hotspots'), { id: 'hotspot-lane' }));
  $('#layer-lanes').replaceChildren(...(s?.layers ?? []).map((l, k) => el('div', { class: 'lane' }, el('span', { class: 'lane-label' }, l.kind),
    block('layer', l, k, l.name || l.id, (cat.kinds[l.kind]?.modifier ? 'modifier ' : '') + (l.muted ? 'muted' : '')))));
  drawWave(t);
}

async function decodeWave(t) {
  try {
    audioCtx ??= new (window.AudioContext || window.webkitAudioContext)();
    const bytes = await (await fetch(t.path)).arrayBuffer();
    const buf = await audioCtx.decodeAudioData(bytes);
    const barSec = 240 / t.bpm;
    const bars = Math.ceil(buf.duration / barSec);
    const px = bars * PPB;
    const data = buf.getChannelData(0);
    const peaks = new Float32Array(px);
    const per = buf.sampleRate * barSec / PPB;
    for (let x = 0; x < px; x++) {
      let m = 0;
      const a = Math.floor(x * per), b = Math.min(data.length, Math.floor((x + 1) * per));
      for (let k = a; k < b; k += 4) m = Math.max(m, Math.abs(data[k]));
      peaks[x] = m;
    }
    waves[t.id] = { peaks, bars, duration: buf.duration };
    if (t === track()) renderTimeline();
  } catch (e) {
    console.warn('waveform', t.path, e);
  }
}

function drawWave(t) {
  const c = $('#wave-canvas');
  const wave = waves[t.id];
  c.width = trackBars(t) * PPB;
  const g = c.getContext('2d');
  g.fillStyle = '#111114'; g.fillRect(0, 0, c.width, c.height);
  if (!wave) { g.fillStyle = '#555'; g.fillText('decoding…', 4, 14); return; }
  g.fillStyle = '#5c7a8c';
  for (let x = 0; x < wave.peaks.length; x++) {
    const h = wave.peaks[x] * c.height;
    g.fillRect(x, (c.height - h) / 2, 1, h);
  }
  g.fillStyle = '#2e2e36';
  for (let k = 0; k <= wave.bars; k += 4) g.fillRect(k * PPB, 0, 1, c.height);
}

boot();
