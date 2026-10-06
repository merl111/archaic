const { avc, ini, esc, spaces, rooms, people, timelines, threadReplies, roomMembers } = window.MX;
const $ = id => document.getElementById(id);
const I = {
  reply: '<svg viewBox="0 0 24 24"><path d="M9 14 4 9l5-5"/><path d="M4 9h11a5 5 0 0 1 5 5v6"/></svg>',
  thread: '<svg viewBox="0 0 24 24"><path d="M4 5h16v10H9l-5 4z"/></svg>',
  more: '<svg viewBox="0 0 24 24"><circle cx="5" cy="12" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/></svg>',
  lock: '<svg viewBox="0 0 24 24"><rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/></svg>',
  x: '<svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6 6 18"/></svg>',
  file: '<svg viewBox="0 0 24 24"><path d="M14 3H6v18h12V7z"/><path d="M14 3v4h4"/></svg>',
  dl: '<svg viewBox="0 0 24 24"><path d="M12 4v11M7 10l5 5 5-5M5 20h14"/></svg>',
  chev: '<svg viewBox="0 0 24 24" style="width:14px;height:14px;color:var(--muted)"><path d="m9 6 6 6-6 6"/></svg>',
};
const state = JSON.parse(localStorage.getItem('nebula-state') || 'null') || { space: 'work', room: 'design', filter: 'all', panel: 'threads' };
state.reply = null;
const save = () => localStorage.setItem('nebula-state', JSON.stringify({ space: state.space, room: state.room, filter: state.filter, panel: state.panel }));
const room = () => rooms.find(r => r.id === state.room);
const avatar = (name, cls = '', online) => `<span class="av ${cls}" style="background:${avc(name)}">${ini(name)}${online ? '<span class="presence"></span>' : ''}</span>`;

function toast(t) { const el = $('toast'); el.textContent = t; el.classList.add('show'); clearTimeout(toast.h); toast.h = setTimeout(() => el.classList.remove('show'), 1800); }

function renderRail() {
  $('rail').innerHTML = spaces.map(s => s.id === 'home'
    ? `<button class="space home ${state.space === 'home' ? 'on' : ''}" aria-label="Home" title="Home" onclick="pickSpace('home')"><svg viewBox="0 0 24 24"><path d="M4 11 12 4l8 7v9h-5v-6H9v6H4z"/></svg></button><div class="sep"></div>`
    : `<button class="space ${state.space === s.id ? 'on' : ''}" style="background:oklch(80% 0.15 ${s.hue})" title="${s.name}" aria-label="${s.name}" data-od-id="space-${s.id}" onclick="pickSpace('${s.id}')">${s.glyph}${s.unread ? `<span class="ub">${s.unread}</span>` : ''}</button>`
  ).join('') + `<button class="add" aria-label="Create a space" onclick="toast('Create or join a space')"><svg viewBox="0 0 24 24"><path d="M12 5v14M5 12h14"/></svg></button>
  <button class="space me" style="background:${avc(people.you.name)}" aria-label="Settings" onclick="toast('Settings · Sessions · Security & privacy')">M</button>`;
}
function pickSpace(id) {
  state.space = id; state.filter = 'all';
  const first = rooms.find(r => id === 'home' || r.space === id); if (first) state.room = first.id;
  renderAll();
}
function renderFilters() {
  const f = [['all', 'All'], ['unread', 'Unread'], ['people', 'People'], ['fav', 'Favourites']];
  $('filters').innerHTML = f.map(([v, l]) => `<button class="chip ${state.filter === v ? 'on' : ''}" onclick="setFilter('${v}')">${l}</button>`).join('');
}
function setFilter(v) { state.filter = v; renderFilters(); renderRooms(); save(); }
function roomRow(r) {
  const av = r.kind === 'dm' ? avatar(r.name, '', r.online) : avatar(r.name, 'sq');
  return `<button class="room ${state.room === r.id ? 'on' : ''} ${r.unread ? 'unread' : ''}" data-od-id="room-${r.id}" onclick="pickRoom('${r.id}')">${av}
    <span class="nm"><b>${r.kind === 'room' ? '# ' : ''}${esc(r.name)}</b><small>${esc(r.last || '')}</small></span>${r.unread ? `<span class="badge ${r.hl ? 'hl' : ''}">${r.unread}</span>` : ''}</button>`;
}
function renderRooms() {
  $('spaceName').textContent = spaces.find(s => s.id === state.space).name;
  let list = rooms.filter(r => state.space === 'home' || r.space === state.space || r.kind === 'dm');
  if (state.filter === 'unread') list = list.filter(r => r.unread);
  if (state.filter === 'people') list = list.filter(r => r.kind === 'dm');
  if (state.filter === 'fav') list = list.filter(r => r.fav);
  const g = (label, arr) => arr.length ? `<div class="group-label">${label}</div>${arr.map(roomRow).join('')}` : '';
  $('roomList').innerHTML = (g('Rooms', list.filter(r => r.kind === 'room')) + g('Direct messages', list.filter(r => r.kind === 'dm')))
    || `<p style="color:var(--muted);padding:16px 8px;margin:0">You’re all caught up.</p>`;
}
function pickRoom(id) { state.room = id; state.reply = null; room().unread = 0; renderAll(); $('input').focus(); }

function body(m) {
  let t = esc(m.txt).replace(/@mathias/g, '<span class="pill-mention">@mathias</span>');
  return t;
}
function msgHTML(m, i, inPanel) {
  if (m.day) return `<div class="day">${m.day}</div>`;
  const p = people[m.by];
  let x = '';
  if (m.file) x += `<div class="file-card"><span class="file-ic">${I.file}</span><span style="flex:1;min-width:0"><b>${esc(m.file.name)}</b><small class="mono">${m.file.size} · encrypted</small></span><button class="icon-btn" aria-label="Download" onclick="toast('Downloading ${esc(m.file.name)}')">${I.dl}</button></div>`;
  if (m.poll) {
    const tot = m.poll.opts.reduce((a, o) => a + o[1], 0), v = m.poll.v != null;
    x += `<div class="poll" data-od-id="poll"><h4>${esc(m.poll.q)}</h4>${m.poll.opts.map((o, k) => `<button class="poll-opt ${m.poll.v === k ? 'voted' : ''}" onclick="vote(${i},${k})"><i style="width:${v ? Math.round(o[1] / tot * 100) : 0}%"></i><span>${esc(o[0])}</span><span class="mono">${v ? Math.round(o[1] / tot * 100) + '%' : ''}</span></button>`).join('')}<small>${v ? tot + ' votes · results visible to all' : 'Vote to see results'}</small></div>`;
  }
  if (m.reacts) x += `<div class="reacts">${m.reacts.map((r, k) => `<button class="react ${r[2] ? 'mine' : ''}" onclick="react(${i},${k})">${r[0]}<span>${r[1]}</span></button>`).join('')}</div>`;
  if (m.thread && !inPanel) x += `<button class="thread-sum" onclick="setPanel('threads')"><span class="stack-av">${m.thread.who.map(w => avatar(people[w].name, 'sm')).join('')}</span><span><b>${m.thread.n} replies</b><small>${esc(m.thread.last)}</small></span></button>`;
  const rq = m.reply ? `<div class="reply-q">${I.reply.replace('<svg', '<svg style="width:13px;height:13px"')}<b>${people[m.reply.by].name}</b>${esc(m.reply.txt)}</div>` : '';
  const bar = inPanel ? '' : `<div class="hover-bar"><button aria-label="React with thumbs up" onclick="quickReact(${i})">👍</button><button aria-label="Reply" onclick="startReply(${i})">${I.reply}</button><button aria-label="Reply in thread" onclick="setPanel('threads')">${I.thread}</button><button aria-label="More options" onclick="toast('Edit · Pin · Forward · View source · Report')">${I.more}</button></div>`;
  return `<div class="msg ${m.cont ? 'cont' : ''} ${m.mention ? 'hlmsg' : ''}" data-od-id="${inPanel ? 'thread' : 'msg'}-${i}">${avatar(p.name)}<div>${m.cont ? '' : `<div class="who"><b style="color:${avc(p.name, 82, .12)}">${p.name}</b><span class="mono">${m.t}</span></div>`}${rq}<p class="txt">${body(m)}</p>${x}</div>${bar}</div>`;
}
function renderTimeline() {
  const r = room();
  $('headAv').innerHTML = r.kind === 'dm' ? avatar(r.name, 'lg', r.online) : avatar(r.name, 'sq lg');
  $('headName').innerHTML = `${r.kind === 'room' ? '# ' : ''}${esc(r.name)}<span class="e2ee">${I.lock}Encrypted</span>`;
  $('headTopic').textContent = r.kind === 'dm' ? (people[r.id]?.id || '') : `${r.members.toLocaleString()} members · ${r.topic}`;
  const tl = timelines[r.id] || [];
  $('timeline').innerHTML = tl.map((m, i) => msgHTML(m, i)).join('');
  $('timeline').scrollTop = 1e6;
  $('typing').innerHTML = r.id === 'design' ? `<span class="dots"><i></i><i></i><i></i></span> Noor is typing…` : '';
  $('input').placeholder = `Message ${r.kind === 'room' ? '#' : ''}${r.name}`;
}
function vote(i, k) { const p = timelines[state.room][i].poll; if (p.v != null) p.opts[p.v][1]--; p.v = k; p.opts[k][1]++; renderTimeline(); }
function react(i, k) { const r = timelines[state.room][i].reacts[k]; r[2] = !r[2]; r[1] += r[2] ? 1 : -1; timelines[state.room][i].reacts = timelines[state.room][i].reacts.filter(x => x[1] > 0); renderTimeline(); }
function quickReact(i) { const m = timelines[state.room][i]; m.reacts = m.reacts || []; const e = m.reacts.find(r => r[0] === '👍'); if (e) { if (!e[2]) { e[2] = true; e[1]++; } } else m.reacts.push(['👍', 1, true]); renderTimeline(); }
function startReply(i) { const m = timelines[state.room][i]; state.reply = { by: m.by, txt: m.txt.split('\n')[0] }; renderReply(); $('input').focus(); }
function renderReply() {
  const el = $('replying');
  el.classList.toggle('show', !!state.reply);
  el.innerHTML = state.reply ? `${I.reply.replace('<svg', '<svg style="width:14px;height:14px"')}Replying to <b>${people[state.reply.by].name}</b><span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;min-width:0">${esc(state.reply.txt)}</span><button class="icon-btn" aria-label="Cancel reply" onclick="state.reply=null;renderReply()">${I.x}</button>` : '';
}
function send() {
  const v = $('input').value.trim(); if (!v) return;
  const tl = timelines[state.room];
  tl.push({ by: 'you', t: new Date().toTimeString().slice(0, 5), txt: v, reply: state.reply || undefined });
  room().last = 'You: ' + v;
  $('input').value = ''; state.reply = null; autosize(); renderReply(); renderTimeline(); renderRooms();
}

function setPanel(p) {
  state.panel = state.panel === p ? null : p; save(); renderPanel();
}
function renderPanel() {
  const el = $('panel'), p = state.panel, r = room();
  el.classList.toggle('hide', !p);
  ['threads', 'members', 'info'].forEach(k => $('btn-' + k).classList.toggle('on', p === k));
  $('panelFoot').innerHTML = '';
  if (!p) return;
  if (p === 'threads') {
    $('panelTitle').textContent = 'Thread';
    const root = (timelines.design || []).find(m => m.thread);
    if (r.id !== 'design' || !root) { $('panelBody').innerHTML = `<p style="color:var(--muted);padding:18px;margin:0">No threads in this room yet. Hover a message and choose “Reply in thread”.</p>`; return; }
    $('panelBody').innerHTML = `<div class="thread-root">${msgHTML(root, 0, true)}</div>` + threadReplies.map((m, i) => msgHTML(m, i, true)).join('');
    $('panelFoot').innerHTML = `<div class="composer"><textarea id="tinput" rows="1" placeholder="Reply in thread…" aria-label="Thread reply"></textarea><button class="send" aria-label="Send reply" onclick="sendThread()"><svg viewBox="0 0 24 24"><path d="M5 12h14M13 6l6 6-6 6"/></svg></button></div>`;
    $('tinput').addEventListener('keydown', e => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); sendThread(); } });
  }
  if (p === 'members') {
    $('panelTitle').textContent = `Members · ${r.kind === 'dm' ? 2 : r.members.toLocaleString()}`;
    const list = r.kind === 'dm' ? [r.id, 'you'] : roomMembers;
    $('panelBody').innerHTML = `<div class="group-label" style="padding:10px 18px 6px">Joined</div>` + list.map(k => { const u = people[k]; return `<button class="member" onclick="toast('${u.name} · verified session')">${avatar(u.name, '', u.online)}<span class="nm"><b>${u.name}</b><small>${u.id}</small></span>${u.pl ? `<span class="pl">${u.pl}</span>` : ''}</button>`; }).join('');
  }
  if (p === 'info') {
    $('panelTitle').textContent = 'Room info';
    $('panelBody').innerHTML = `<div class="info-hero">${avatar(r.name, 'sq')}<h4>${r.kind === 'room' ? '#' : ''}${esc(r.name)}</h4><p>${esc(r.topic || people[r.id]?.id || '')}</p></div>
      ${[['Files', '12'], ['Pinned messages', '2'], ['Notifications', 'Mentions'], ['Export chat', ''], ['Settings', '']].map(([a, b]) => `<button class="info-row" onclick="toast('${a}')"><span>${a}</span><small>${b}</small>${I.chev}</button>`).join('')}
      <div style="padding:14px 18px"><span class="e2ee">${I.lock}Messages are end-to-end encrypted</span></div>`;
  }
}
function sendThread() {
  const v = $('tinput').value.trim(); if (!v) return;
  threadReplies.push({ by: 'you', t: new Date().toTimeString().slice(0, 5), txt: v });
  const root = timelines.design.find(m => m.thread); root.thread.n++; root.thread.last = 'You: ' + v;
  if (!root.thread.who.includes('you')) root.thread.who.push('you');
  renderPanel(); renderTimeline();
}

let callT = null;
function startCall() {
  const r = room(); $('callRoom').textContent = (r.kind === 'room' ? '#' : '') + r.name;
  const who = r.kind === 'dm' ? [r.id, 'you'] : ['ana', 'kai', 'ben', 'you'];
  $('callGrid').innerHTML = who.map((k, i) => `<div class="tile ${i === 0 ? 'speaking' : ''}" style="background:${avc(people[k].name, 72, .13)}">${avatar(people[k].name)}<small>${k === 'you' ? 'You' : people[k].name.split(' ')[0]}</small></div>`).join('');
  $('call').classList.add('show');
  let s = 0; clearInterval(callT);
  callT = setInterval(() => { s++; $('callTime').textContent = String(s / 60 | 0).padStart(2, '0') + ':' + String(s % 60).padStart(2, '0');
    const tiles = $('callGrid').children; [...tiles].forEach((t, i) => t.classList.toggle('speaking', i === (s >> 1) % tiles.length)); }, 1000);
}
function endCall() { clearInterval(callT); $('call').classList.remove('show'); $('callTime').textContent = '00:00'; toast('Call ended'); }

function autosize() { const t = $('input'); t.style.height = '32px'; t.style.height = Math.min(t.scrollHeight, 140) + 'px'; $('sendBtn').disabled = !t.value.trim(); }
$('input').addEventListener('input', autosize);
$('input').addEventListener('keydown', e => {
  if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); send(); }
  if (e.key === 'Escape') { state.reply = null; renderReply(); }
});
document.addEventListener('keydown', e => { if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); toast('Search rooms, people and messages'); } });

function renderAll() { renderRail(); renderFilters(); renderRooms(); renderTimeline(); renderReply(); renderPanel(); save(); }
renderAll();
