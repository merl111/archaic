const { avc, ini, esc, spaces, rooms, people, timelines, roomMembers } = window.MX;
const $ = id => document.getElementById(id);
const st = JSON.parse(localStorage.getItem('daylight-state') || 'null') || { space: 'work', room: 'design', ctx: 'about', ctxOpen: true, verified: false };
st.reply = null;
const save = () => localStorage.setItem('daylight-state', JSON.stringify({ ...st, reply: null }));
const cur = () => rooms.find(r => r.id === st.room);
const av = (name, cls = '', dot) => `<span class="av ${cls}" style="background:${avc(name, 84, .12)}">${ini(name)}${dot ? '<span class="dot"></span>' : ''}</span>`;
function toast(t) { const el = $('toast'); el.textContent = t; el.classList.add('show'); clearTimeout(toast.h); toast.h = setTimeout(() => el.classList.remove('show'), 1800); }
const RE = '<svg viewBox="0 0 24 24"><path d="M9 14 4 9l5-5"/><path d="M4 9h11a5 5 0 0 1 5 5v6"/></svg>';

function renderTabs() {
  $('tabs').innerHTML = spaces.map(s => `<button class="tab ${st.space === s.id ? 'on' : ''}" data-od-id="tab-${s.id}" onclick="pickSpace('${s.id}')">
    <span class="g" style="background:${s.hue != null ? `oklch(85% 0.12 ${s.hue})` : 'var(--soft)'}">${s.glyph || '<svg viewBox="0 0 24 24" style="width:14px;height:14px"><path d="M4 11 12 4l8 7v9h-5v-6H9v6H4z"/></svg>'}</span>${s.name}${s.unread ? `<span class="n">${s.unread}</span>` : ''}</button>`).join('');
  const me = $('meAv'); me.style.background = avc(people.you.name, 84, .12); me.textContent = ini(people.you.name);
}
function pickSpace(id) { st.space = id; const f = rooms.find(r => id === 'home' || r.space === id); if (f) st.room = f.id; renderAll(); }

function renderRooms() {
  $('spaceTitle').textContent = spaces.find(s => s.id === st.space).name;
  const q = $('q').value.trim().toLowerCase();
  let list = rooms.filter(r => (st.space === 'home' || r.space === st.space || r.kind === 'dm') && r.name.toLowerCase().includes(q));
  const row = r => `<button class="room ${st.room === r.id ? 'on' : ''}" data-od-id="room-${r.id}" onclick="pickRoom('${r.id}')">${r.kind === 'dm' ? av(r.name, 'round', r.online) : av(r.name)}
    <span class="nm"><b>${r.kind === 'room' ? '#' : ''}${esc(r.name)}</b><small>${esc(r.last || '')}</small></span>
    <span class="side"><span class="mono">${(timelines[r.id] || []).filter(m => m.t).slice(-1)[0]?.t || ''}</span>${r.unread ? `<span class="cnt ${r.hl ? 'hl' : ''}">${r.unread}</span>` : ''}</span></button>`;
  const fav = list.filter(r => r.fav), rest = list.filter(r => !r.fav);
  $('rl').innerHTML = (fav.length ? `<div class="sec">Pinned</div>${fav.map(row).join('')}` : '')
    + (rest.filter(r => r.kind === 'room').length ? `<div class="sec">Rooms</div>${rest.filter(r => r.kind === 'room').map(row).join('')}` : '')
    + (rest.filter(r => r.kind === 'dm').length ? `<div class="sec">People</div>${rest.filter(r => r.kind === 'dm').map(row).join('')}` : '')
    || `<p style="color:var(--muted);padding:14px 8px;margin:0">No rooms match “${esc(q)}”.</p>`;
}
function pickRoom(id) { st.room = id; st.reply = null; cur().unread = 0; renderAll(); $('input').focus(); }

function renderStream() {
  const r = cur();
  $('hAv').innerHTML = r.kind === 'dm' ? av(r.name, 'round', r.online) : av(r.name);
  $('hName').textContent = (r.kind === 'room' ? '#' : '') + r.name;
  $('hSub').textContent = r.kind === 'dm' ? `${people[r.id].id} · ${r.online ? 'Online' : 'Away'} · encrypted` : `${r.members.toLocaleString()} members · encrypted · ${r.topic}`;
  $('input').placeholder = `Message ${r.kind === 'room' ? '#' : ''}${r.name}`;
  const tl = timelines[r.id] || []; let html = '', grp = null;
  const close = () => { if (grp) html += '</div></div>'; grp = null; };
  tl.forEach((m, i) => {
    if (m.day) { close(); html += `<div class="day">${m.day}</div>`; return; }
    if (!grp || grp !== m.by || m.thread) { close(); grp = m.by; const p = people[m.by];
      html += `<div class="grp ${m.by === 'you' ? 'me' : ''}">${m.by === 'you' ? '' : av(p.name, 'sm')}<div class="col">${m.by === 'you' || r.kind === 'dm' ? '' : `<div class="sender" style="color:${avc(p.name, 45, .14)}">${p.name}</div>`}`; }
    let inner = esc(m.txt).replace(/@mathias/g, '<span class="mention">@mathias</span>');
    if (m.reply) inner = `<span class="rq"><b>${people[m.reply.by].name}</b> · ${esc(m.reply.txt)}</span>` + inner;
    if (m.file) inner += `<span class="attach"><span class="fi"><svg viewBox="0 0 24 24"><path d="M14 3H6v18h12V7z"/><path d="M14 3v4h4"/></svg></span><span><b>${esc(m.file.name)}</b><small class="mono">${m.file.size}</small></span></span>`;
    let cls = 'bub';
    if (m.poll) { cls += ' pollb'; const tot = m.poll.opts.reduce((a, o) => a + o[1], 0), v = m.poll.v != null;
      inner = `<h4>${esc(m.poll.q)}</h4>` + m.poll.opts.map((o, k) => `<button class="po ${m.poll.v === k ? 'voted' : ''}" onclick="vote(${i},${k})"><i style="width:${v ? Math.round(o[1] / tot * 100) : 0}%"></i><span>${esc(o[0])}</span><span class="mono">${v ? o[1] : ''}</span></button>`).join('') + `<small>${v ? tot + ' votes' : 'Tap an option to vote'}</small>`; }
    if (m.thread) inner += `<span style="display:block;margin-top:8px;font-size:12.5px;font-weight:700">${m.thread.n} replies in thread →</span>`;
    html += `<div class="${cls}" data-od-id="bubble-${i}">${inner}<span class="t">${m.t}</span><span class="quick"><button aria-label="React heart" onclick="quick(${i},'❤️')">❤️</button><button aria-label="React laugh" onclick="quick(${i},'😂')">😂</button><button aria-label="Reply" onclick="reply(${i})">${RE}</button></span></div>`;
    if (m.reacts?.length) html += `<div class="reacts">${m.reacts.map((x, k) => `<button class="react ${x[2] ? 'mine' : ''}" onclick="react(${i},${k})">${x[0]}<span>${x[1]}</span></button>`).join('')}</div>`;
  });
  close();
  $('stream').innerHTML = html; $('stream').scrollTop = 1e6;
}
function vote(i, k) { const p = timelines[st.room][i].poll; if (p.v != null) p.opts[p.v][1]--; p.v = k; p.opts[k][1]++; renderStream(); }
function react(i, k) { const m = timelines[st.room][i], x = m.reacts[k]; x[2] = !x[2]; x[1] += x[2] ? 1 : -1; m.reacts = m.reacts.filter(y => y[1] > 0); renderStream(); }
function quick(i, e) { const m = timelines[st.room][i]; m.reacts = m.reacts || []; const x = m.reacts.find(y => y[0] === e); if (!x) m.reacts.push([e, 1, true]); else if (!x[2]) { x[2] = true; x[1]++; } renderStream(); }
function reply(i) { const m = timelines[st.room][i]; st.reply = { by: m.by, txt: m.txt.split('\n')[0] }; renderReply(); $('input').focus(); }
function renderReply() { const b = $('replyBar'); b.classList.toggle('show', !!st.reply);
  b.innerHTML = st.reply ? `Replying to <b>${people[st.reply.by].name}</b><span style="white-space:nowrap;overflow:hidden;text-overflow:ellipsis;min-width:0">${esc(st.reply.txt)}</span><button class="ibtn" aria-label="Cancel reply" onclick="st.reply=null;renderReply()"><svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6 6 18"/></svg></button>` : ''; }
function send() { const v = $('input').value.trim(); if (!v) return;
  timelines[st.room].push({ by: 'you', t: new Date().toTimeString().slice(0, 5), txt: v, reply: st.reply || undefined });
  cur().last = 'You: ' + v; $('input').value = ''; st.reply = null; size(); renderReply(); renderStream(); renderRooms(); }

function toggleCtx() { st.ctxOpen = !st.ctxOpen; layoutCtx(); save(); }
function layoutCtx() { $('ctx').style.display = st.ctxOpen ? '' : 'none'; document.querySelector('.work').style.gridTemplateColumns = st.ctxOpen ? '300px 1fr 300px' : '300px 1fr'; $('ctxBtn').classList.toggle('on', st.ctxOpen); }
function ctxTab(t) { st.ctx = t; save(); renderCtx(); }
function renderCtx() {
  document.querySelectorAll('.ctx-tab').forEach(b => b.classList.toggle('on', b.dataset.t === st.ctx));
  const r = cur(), b = $('ctxBody');
  if (st.ctx === 'about') b.innerHTML = `<div class="ctx-hero">${av(r.name, r.kind === 'dm' ? 'round' : '')}<h3>${r.kind === 'room' ? '#' : ''}${esc(r.name)}</h3><p>${esc(r.topic || people[r.id]?.id || '')}</p></div>
    <div class="facts"><div class="fact"><b class="num">${r.kind === 'dm' ? 2 : r.members.toLocaleString()}</b><small>Members</small></div><div class="fact"><b>E2EE</b><small>Encryption</small></div></div>
    ${[['Notifications', 'Mentions & keywords'], ['Pinned messages', '2'], ['Threads', r.id === 'design' ? '1 active' : 'None'], ['Room settings', ''], ['Leave room', '']].map(([a, c]) => `<button class="row-btn" onclick="toast('${a}')">${a}<small>${c}</small></button>`).join('')}`;
  if (st.ctx === 'people') { const list = r.kind === 'dm' ? [r.id, 'you'] : roomMembers;
    b.innerHTML = list.map(k => { const u = people[k]; return `<button class="mem" onclick="toast('Open ${u.name}')">${av(u.name, 'round', u.online)}<span class="nm"><b>${u.name}</b><small>${u.id}</small></span>${u.pl ? `<span class="lv">${u.pl}</span>` : ''}</button>`; }).join('') + `<button class="row-btn" style="margin-top:8px" onclick="toast('Invite by Matrix ID or email')">Invite people<small>+</small></button>`; }
  if (st.ctx === 'media') b.innerHTML = `<p style="margin:0 0 10px;color:var(--muted);font-size:12.5px">Shared files in this room (sample swatches stand in for image thumbnails).</p><div class="swatches">${['call-pip-v3', 'composer-states', 'room-list', 'verify-flow', 'tokens', 'whiteboard'].map((n, i) => `<button class="sw" style="background:oklch(86% 0.1 ${i * 55 + 20})" onclick="toast('${n}')">${n}</button>`).join('')}</div>`;
}

const SAS = [['🐶', 'Dog'], ['🔑', 'Key'], ['🌵', 'Cactus'], ['🎸', 'Guitar'], ['🚀', 'Rocket'], ['🍄', 'Mushroom'], ['⚓', 'Anchor']];
function openVerify() { if (st.verified) { toast('This session is verified'); return; } $('emojis').innerHTML = SAS.map(([e, n]) => `<div class="emo"><span>${e}</span><small>${n}</small></div>`).join(''); $('scrim').classList.add('show'); }
function closeVerify() { $('scrim').classList.remove('show'); }
function verified() { st.verified = true; save(); closeVerify(); renderVerify(); toast('Session verified — encrypted history unlocked'); }
function renderVerify() { const c = $('verifyChip'); c.classList.toggle('done', st.verified); c.querySelector('span').textContent = st.verified ? 'Session verified' : 'Verify this session';
  c.querySelector('svg').innerHTML = st.verified ? '<path d="M12 3 4 6v6c0 5 3.5 8 8 9 4.5-1 8-4 8-9V6z"/><path d="m9 12 2 2 4-4"/>' : '<path d="M12 3 4 6v6c0 5 3.5 8 8 9 4.5-1 8-4 8-9V6z"/><path d="M12 8v4M12 15.5v.01"/>'; }

function size() { const t = $('input'); t.style.height = '38px'; t.style.height = Math.min(t.scrollHeight, 140) + 'px'; $('sendBtn').disabled = !t.value.trim(); }
$('input').addEventListener('input', size);
$('input').addEventListener('keydown', e => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); send(); } if (e.key === 'Escape') { st.reply = null; renderReply(); } });
$('q').addEventListener('input', renderRooms);
document.addEventListener('keydown', e => { if (e.key === 'Escape') closeVerify(); });

function renderAll() { renderTabs(); renderRooms(); renderStream(); renderReply(); renderCtx(); layoutCtx(); renderVerify(); save(); }
renderAll();
