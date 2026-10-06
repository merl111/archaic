const { avc, ini, esc, spaces, rooms, people, timelines } = window.MX;
const $ = id => document.getElementById(id);
const st = JSON.parse(localStorage.getItem('tiles-state') || 'null') || { layout: 3, tiles: ['design', 'ana', 'eng', 'mhq'], focus: 0, space: 'home' };
const replies = {};
const save = () => localStorage.setItem('tiles-state', JSON.stringify(st));
const R = id => rooms.find(r => r.id === id);
const sw = r => `oklch(82% 0.13 ${r.kind === 'dm' ? MX.hue(r.name) : (spaces.find(s => s.id === r.space)?.hue ?? 250)})`;
function toast(t) { const el = $('toast'); el.textContent = t; el.classList.add('show'); clearTimeout(toast.h); toast.h = setTimeout(() => el.classList.remove('show'), 1800); }
const IC = {
  reply: '<svg viewBox="0 0 24 24"><path d="M9 14 4 9l5-5"/><path d="M4 9h11a5 5 0 0 1 5 5v6"/></svg>',
  x: '<svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6 6 18"/></svg>',
  max: '<svg viewBox="0 0 24 24"><path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/></svg>',
  call: '<svg viewBox="0 0 24 24"><rect x="3" y="6" width="13" height="12" rx="2"/><path d="m16 10 5-3v10l-5-3"/></svg>',
  file: '<svg viewBox="0 0 24 24"><path d="M14 3H6v18h12V7z"/><path d="M14 3v4h4"/></svg>',
};
const LAYOUTS = [
  [1, '<svg viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" rx="2"/></svg>', 'Single'],
  [2, '<svg viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" rx="2"/><path d="M12 4v16"/></svg>', 'Split'],
  [3, '<svg viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" rx="2"/><path d="M13 4v16M13 12h7"/></svg>', 'Main + two'],
  [4, '<svg viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" rx="2"/><path d="M12 4v16M4 12h16"/></svg>', 'Quad'],
];

/* sidebar */
function renderSide() {
  $('meAv').style.background = avc(people.you.name, 84, .12); $('meAv').textContent = ini(people.you.name);
  $('spc').innerHTML = spaces.map(s => `<button class="${st.space === s.id ? 'on' : ''}" onclick="st.space='${s.id}';save();renderSide()">${s.id === 'home' ? 'All' : s.name}</button>`).join('');
  const vis = st.tiles.slice(0, st.layout);
  const row = r => { const slot = vis.indexOf(r.id);
    return `<button class="ri ${r.unread ? 'unread' : ''} ${slot > -1 ? 'open' : ''}" data-od-id="room-${r.id}" onclick="openRoom('${r.id}', event.shiftKey)" title="Click: open in focused tile · Shift-click: new tile">
    <span class="sw ${r.kind === 'dm' ? 'dm' : ''}" style="background:${sw(r)}"></span><b>${r.kind === 'room' ? '#' : ''}${esc(r.name)}</b>
    ${slot > -1 ? `<span class="pin">T${slot + 1}</span>` : ''}${r.unread ? `<span class="c ${r.hl ? 'hl' : ''}">${r.unread}</span>` : ''}</button>`; };
  const list = rooms.filter(r => st.space === 'home' || r.space === st.space);
  const rm = list.filter(r => r.kind === 'room'), dm = rooms.filter(r => r.kind === 'dm');
  $('list').innerHTML = (rm.length ? `<div class="sec">Rooms</div>${rm.map(row).join('')}` : '') + `<div class="sec">Direct</div>${dm.map(row).join('')}`;
}

/* layout */
function renderSeg() {
  $('seg').innerHTML = LAYOUTS.map(([n, ic, l]) => `<button class="${st.layout === n ? 'on' : ''}" aria-label="${l} layout" title="${l}" onclick="setLayout(${n})">${ic}</button>`).join('');
}
function setLayout(n) { st.layout = n; if (st.focus >= n) st.focus = 0; renderAll(); }

/* tiles */
function msg(m, i, rid) {
  const p = people[m.by];
  let body = `<p class="tx">${esc(m.txt).replace(/@mathias/g, '<span class="mention">@mathias</span>')}</p>`;
  if (m.reply) body = `<span class="q"><b>${people[m.reply.by].name}</b> · ${esc(m.reply.txt)}</span>` + body;
  if (m.file) body += `<button class="file" onclick="toast('Download ${esc(m.file.name)}')">${IC.file}<span><b>${esc(m.file.name)}</b><small class="mono">${m.file.size}</small></span></button>`;
  if (m.poll) { const tot = m.poll.opts.reduce((a, o) => a + o[1], 0), v = m.poll.v != null;
    body += `<div class="poll"><h4>${esc(m.poll.q)}</h4>${m.poll.opts.map((o, k) => `<button class="po ${m.poll.v === k ? 'voted' : ''}" onclick="vote('${rid}',${i},${k})"><i style="width:${v ? Math.round(o[1] / tot * 100) : 0}%"></i><span>${esc(o[0])}</span><span class="mono">${v ? Math.round(o[1] / tot * 100) + '%' : ''}</span></button>`).join('')}<small>${v ? tot + ' votes' : 'Vote to see results'}</small></div>`; }
  if (m.thread) body += `<br><button class="thr" onclick="toast('Thread opens as a tile split')">${m.thread.n} replies · ${esc(m.thread.last)}</button>`;
  if (m.reacts?.length) body += `<div class="rx">${m.reacts.map((x, k) => `<button class="${x[2] ? 'mine' : ''}" onclick="react('${rid}',${i},${k})">${x[0]}<span class="mono">${x[1]}</span></button>`).join('')}</div>`;
  const hov = `<span class="hov"><button aria-label="React thumbs up" onclick="quick('${rid}',${i},'👍')">👍</button><button aria-label="React eyes" onclick="quick('${rid}',${i},'👀')">👀</button><button aria-label="Reply" onclick="reply('${rid}',${i})">${IC.reply}</button></span>`;
  return `<div class="m ${m.cont ? 'cont' : ''} ${m.mention ? 'hlm' : ''}"><span class="av" style="background:${avc(p.name, 84, .12)}">${ini(p.name)}</span><div>${m.cont ? '' : `<div class="who"><b>${p.name}</b><span class="mono">${m.t}</span></div>`}${body}</div>${hov}</div>`;
}
function tileHTML(rid, slot) {
  const r = R(rid);
  const sub = r.kind === 'dm' ? `${people[r.id].id} · ${r.online ? 'online' : 'away'}` : `${r.members.toLocaleString()} members · ${r.topic}`;
  return `<section class="tile ${st.focus === slot ? 'focus' : ''}" data-od-id="tile-${slot + 1}" data-slot="${slot}" onmousedown="setFocus(${slot})">
    <header class="th"><span class="strip" style="background:${sw(r)}"></span><b>${r.kind === 'room' ? '#' : ''}${esc(r.name)}</b><small>${esc(sub)}</small><span class="k">Alt ${slot + 1}</span>
      <button class="tb" aria-label="Call" onclick="toast('Starting call in ${esc(r.name)}')">${IC.call}</button>
      <button class="tb" aria-label="Maximize tile" onclick="maximize(${slot})">${IC.max}</button>
      ${st.layout > 1 ? `<button class="tb" aria-label="Close tile" onclick="closeTile(${slot})">${IC.x}</button>` : ''}</header>
    <div class="tl" id="tl-${slot}"></div>
    <div class="rbar" id="rb-${slot}"></div>
    <div class="cp"><textarea id="in-${slot}" rows="1" aria-label="Message ${esc(r.name)}" placeholder="Message ${r.kind === 'room' ? '#' : ''}${esc(r.name)}"></textarea><button class="send" id="sd-${slot}" disabled onclick="send(${slot})">Send</button></div>
  </section>`;
}
function renderTL(slot) {
  const rid = st.tiles[slot], el = $('tl-' + slot); if (!el) return;
  el.innerHTML = (timelines[rid] || []).map((m, i) => m.day ? `<div class="day">${m.day}</div>` : msg(m, i, rid)).join('');
  el.scrollTop = 1e6;
  const rb = $('rb-' + slot), rp = replies[rid];
  rb.classList.toggle('show', !!rp);
  rb.innerHTML = rp ? `<b>↳ ${people[rp.by].name}</b><span>${esc(rp.txt)}</span><button class="tb" aria-label="Cancel reply" onclick="delete replies['${rid}'];renderTL(${slot})">${IC.x}</button>` : '';
}
function renderGrid() {
  const g = $('grid'); g.className = 'grid l' + st.layout;
  let html = '';
  for (let s = 0; s < st.layout; s++) html += st.tiles[s] ? tileHTML(st.tiles[s], s) : `<button class="empty" onclick="setFocus(${s});openPal()"><span><b>Empty tile</b><br>Press <kbd>Ctrl K</kbd> to pick a room</span></button>`;
  g.innerHTML = html;
  for (let s = 0; s < st.layout; s++) if (st.tiles[s]) { renderTL(s); wire(s); }
  $('wsName').textContent = ['Focus', 'Pair', 'Crit day', 'Mission control'][st.layout - 1];
}
function wire(s) {
  const t = $('in-' + s), b = $('sd-' + s);
  t.addEventListener('input', () => { t.style.height = '30px'; t.style.height = Math.min(t.scrollHeight, 120) + 'px'; b.disabled = !t.value.trim(); });
  t.addEventListener('keydown', e => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); send(s); } if (e.key === 'Escape') { delete replies[st.tiles[s]]; renderTL(s); } });
  t.addEventListener('focus', () => setFocus(s));
}
function setFocus(s) { if (st.focus === s) return; st.focus = s; document.querySelectorAll('.tile').forEach(t => t.classList.toggle('focus', +t.dataset.slot === s)); save(); }
function focusInput(s) { setFocus(s); $('in-' + s)?.focus({ preventScroll: true }); }
function openRoom(id, newTile) {
  R(id).unread = 0;
  const vis = st.tiles.slice(0, st.layout), at = vis.indexOf(id);
  if (at > -1) { renderSide(); return focusInput(at); }
  if (newTile && st.layout < 4) { st.layout++; const s = st.layout - 1; st.tiles[s] = id; st.focus = s; }
  else st.tiles[st.focus] = id;
  renderAll(); focusInput(st.focus);
}
function closeTile(s) { const [r] = st.tiles.splice(s, 1); st.tiles.push(r); st.layout = Math.max(1, st.layout - 1); st.focus = 0; renderAll(); }
function maximize(s) { if (st.layout === 1) return setLayout(3); const [r] = st.tiles.splice(s, 1); st.tiles.unshift(r); st.focus = 0; setLayout(1); }

function send(s) { const t = $('in-' + s), v = t.value.trim(); if (!v) return; const rid = st.tiles[s];
  timelines[rid].push({ by: 'you', t: new Date().toTimeString().slice(0, 5), txt: v, reply: replies[rid] });
  delete replies[rid]; t.value = ''; t.style.height = '30px'; $('sd-' + s).disabled = true; refresh(rid); }
function refresh(rid) { st.tiles.slice(0, st.layout).forEach((r, s) => r === rid && renderTL(s)); }
function vote(rid, i, k) { const p = timelines[rid][i].poll; if (p.v != null) p.opts[p.v][1]--; p.v = k; p.opts[k][1]++; refresh(rid); }
function react(rid, i, k) { const m = timelines[rid][i], x = m.reacts[k]; x[2] = !x[2]; x[1] += x[2] ? 1 : -1; m.reacts = m.reacts.filter(y => y[1] > 0); refresh(rid); }
function quick(rid, i, e) { const m = timelines[rid][i]; m.reacts = m.reacts || []; const x = m.reacts.find(y => y[0] === e); if (!x) m.reacts.push([e, 1, true]); else if (!x[2]) { x[2] = true; x[1]++; } refresh(rid); }
function reply(rid, i) { const m = timelines[rid][i]; replies[rid] = { by: m.by, txt: m.txt.split('\n')[0] }; refresh(rid); const s = st.tiles.indexOf(rid); if (s > -1) focusInput(s); }

/* command palette */
const CMDS = [
  ['Layout: single', 'Focus on one room', () => setLayout(1)],
  ['Layout: split', 'Two rooms side by side', () => setLayout(2)],
  ['Layout: main + two', 'One large tile, two stacked', () => setLayout(3)],
  ['Layout: quad', 'Four rooms at once', () => setLayout(4)],
  ['Mark all as read', 'Clear unread badges', () => { rooms.forEach(r => r.unread = 0); renderSide(); toast('All rooms marked read'); }],
  ['Start call', 'In the focused tile', () => toast('Starting call in ' + R(st.tiles[st.focus]).name)],
  ['Create room', 'New encrypted room', () => toast('Create room')],
  ['Verify this session', 'Cross-sign with another device', () => toast('Verification request sent to your other sessions')],
];
let pal = [], sel = 0;
function openPal() { $('scrim').classList.add('show'); $('pq').value = ''; renderPal(); $('pq').focus(); }
function closePal() { $('scrim').classList.remove('show'); }
function renderPal() {
  const q = $('pq').value.trim().toLowerCase();
  const rm = rooms.filter(r => r.name.toLowerCase().includes(q) || (r.topic || '').toLowerCase().includes(q)).map(r => ({ g: r.kind === 'dm' ? 'People' : 'Rooms', r, l: (r.kind === 'room' ? '#' : '') + r.name, s: r.kind === 'dm' ? people[r.id].id : r.topic }));
  const cm = CMDS.filter(c => (c[0] + c[1]).toLowerCase().includes(q)).map(c => ({ g: 'Commands', c, l: c[0], s: c[1] }));
  pal = q ? [...rm, ...cm] : [...rm.slice(0, 5), ...cm.slice(0, 5)];
  sel = Math.min(sel, Math.max(0, pal.length - 1));
  let g = '';
  $('plist').innerHTML = pal.map((it, i) => { const head = it.g !== g ? `<div class="pg">${g = it.g}</div>` : '';
    return head + `<button class="pi ${i === sel ? 'sel' : ''}" role="option" aria-selected="${i === sel}" onmousemove="if(sel!==${i}){sel=${i};renderPal()}" onclick="runPal(${i}, event.shiftKey)">
      ${it.r ? `<span class="sw" style="width:10px;height:10px;border:1.5px solid currentColor;border-radius:${it.r.kind === 'dm' ? '50%' : '3px'};background:${sw(it.r)}"></span>` : '<kbd>›</kbd>'}<b>${esc(it.l)}</b><small>${esc(it.s || '')}</small>${it.r && it.r.unread ? `<span class="mono">${it.r.unread} new</span>` : ''}</button>`; }).join('')
    || `<p style="padding:14px;margin:0;color:var(--muted)">Nothing matches “${esc(q)}”.</p>`;
}
function runPal(i, shift) { const it = pal[i]; if (!it) return; closePal(); it.r ? openRoom(it.r.id, shift) : it.c[2](); }
$('pq').addEventListener('input', () => { sel = 0; renderPal(); });
$('pq').addEventListener('keydown', e => {
  if (e.key === 'ArrowDown') { e.preventDefault(); sel = (sel + 1) % pal.length; renderPal(); }
  if (e.key === 'ArrowUp') { e.preventDefault(); sel = (sel - 1 + pal.length) % pal.length; renderPal(); }
  if (e.key === 'Enter') { e.preventDefault(); runPal(sel, e.shiftKey); }
});
document.addEventListener('keydown', e => {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); $('scrim').classList.contains('show') ? closePal() : openPal(); }
  if (e.key === 'Escape' && $('scrim').classList.contains('show')) closePal();
  if (e.altKey && /^[1-4]$/.test(e.key)) { const s = +e.key - 1; if (s < st.layout && st.tiles[s]) { e.preventDefault(); focusInput(s); } }
});

function renderAll() { renderSeg(); renderGrid(); renderSide(); save(); }
renderAll();
