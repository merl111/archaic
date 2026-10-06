// Shared demo data for all three concept prototypes.
// Fictional people & rooms on illustrative homeservers — sample content only.
window.MX = (() => {
  const hue = s => [...s].reduce((a, c) => a + c.charCodeAt(0) * 7, 0) % 360;
  const avc = (s, l = 80, c = 0.14) => `oklch(${l}% ${c} ${hue(s)})`;
  const ini = s => s.replace(/^[#@!]/, '').slice(0, 1).toUpperCase();
  const esc = s => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

  const spaces = [
    { id: 'home', name: 'Home' },
    { id: 'work', name: 'Lumen Labs', hue: 40, glyph: 'L', unread: 12 },
    { id: 'matrix', name: 'Matrix Community', hue: 165, glyph: 'M', unread: 3 },
    { id: 'fosdem', name: 'FOSDEM Devroom', hue: 300, glyph: 'F' },
    { id: 'games', name: 'Game night', hue: 95, glyph: 'G' },
  ];

  const rooms = [
    { id: 'design', space: 'work', name: 'design-crit', kind: 'room', unread: 4, hl: true, fav: true, members: 14, topic: 'Weekly critique · Fridays 15:00 · bring WIP, not finals', last: 'Kai: lead with the emoji compare' },
    { id: 'eng', space: 'work', name: 'engineering', kind: 'room', unread: 8, members: 42, topic: 'Builds, deploys and on-call handoffs', last: 'Lia: cold start feels much faster' },
    { id: 'rand', space: 'work', name: 'random', kind: 'room', members: 58, topic: 'Off-topic, pets, lunch spots', last: 'Ben: it is on a sabbatical' },
    { id: 'mhq', space: 'matrix', name: 'Matrix HQ', kind: 'room', unread: 3, members: 9120, topic: 'Chat about Matrix, the open standard', last: 'Lia: anyone tracking extensible events?' },
    { id: 'ew', space: 'matrix', name: 'Client dev', kind: 'room', members: 1840, topic: 'Building Matrix clients', last: 'Kai: sliding sync notes are up' },
    { id: 'talks', space: 'fosdem', name: 'speakers', kind: 'room', members: 31, topic: 'Devroom logistics', last: 'Noor: schedule is final' },
    { id: 'ana', space: null, name: 'Ana Ribeiro', kind: 'dm', online: true, unread: 1, hl: true, fav: true, last: 'Can you look at the call tile?' },
    { id: 'kai', space: null, name: 'Kai Tanaka', kind: 'dm', online: true, last: 'Merged, thanks!' },
    { id: 'noor', space: null, name: 'Noor Haddad', kind: 'dm', last: 'Shared whiteboard.png' },
  ];

  const people = {
    ana: { name: 'Ana Ribeiro', id: '@ana:lumen.chat', pl: 'Admin', online: true },
    kai: { name: 'Kai Tanaka', id: '@kai:lumen.chat', pl: 'Moderator', online: true },
    noor: { name: 'Noor Haddad', id: '@noor:matrix.org' },
    ben: { name: 'Ben Okafor', id: '@ben:lumen.chat', online: true },
    lia: { name: 'Lia Novak', id: '@lia:fosdem.org' },
    you: { name: 'Mathias', id: '@mathias:lumen.chat', pl: 'Admin', online: true },
  };

  const timelines = {
    design: [
      { day: 'Yesterday' },
      { by: 'kai', t: '17:42', txt: 'Pushed room list density options to staging. Compact mode drops the preview line.' },
      { by: 'noor', t: '17:44', txt: 'Compact looks great on a 13" screen. Can we keep unread rooms bold though?', reacts: [['💯', 3, false], ['👀', 1, false]] },
      { day: 'Today' },
      { by: 'ana', t: '09:12', txt: 'Morning! Three things for crit today:\n1. Composer states — idle, replying, editing\n2. Call tile picture-in-picture\n3. Verification flow copy', thread: { n: 3, who: ['kai', 'noor', 'ben'], last: 'Ben: will pick up the call tile' } },
      { by: 'ben', t: '09:20', txt: 'Mocked the PiP tile, export attached.', file: { name: 'call-pip-v3.fig', size: '4.2 MB' } },
      { by: 'ben', t: '09:21', cont: true, txt: 'Quick vote before we start:', poll: { q: 'Default call layout?', opts: [['Speaker focus', 4], ['Grid', 6], ['Spotlight + filmstrip', 2]], v: null } },
      { by: 'kai', t: '09:34', reply: { by: 'ana', txt: '3. Verification flow copy' }, txt: 'For verification I would lead with the emoji compare — it is the one people actually finish. @mathias can you take a pass?', mention: true, reacts: [['🙌', 2, true]] },
    ],
    eng: [{ day: 'Today' }, { by: 'kai', t: '08:01', txt: 'matrix-rust-sdk bump merged. Sliding sync is on by default for new logins.' }, { by: 'lia', t: '08:15', txt: 'Cold start feels much faster on my test account.', reacts: [['🚀', 4, false]] }],
    rand: [{ day: 'Today' }, { by: 'noor', t: '12:30', txt: 'Who took the good kettle from the 3rd floor' }, { by: 'ben', t: '12:31', txt: 'it is on a sabbatical' }],
    mhq: [{ day: 'Today' }, { by: 'lia', t: '10:02', txt: 'Is anyone tracking the extensible events proposal?' }],
    ew: [{ day: 'Today' }, { by: 'kai', t: '11:00', txt: 'Sliding sync notes are up in the room topic.' }],
    talks: [{ day: 'Mon' }, { by: 'noor', t: '16:10', txt: 'Schedule is final. Slides due the day before.' }],
    ana: [{ day: 'Today' }, { by: 'ana', t: '09:40', txt: 'Can you look at the call tile? The speaking ring feels too thin at 1×.' }],
    kai: [{ day: 'Today' }, { by: 'you', t: '08:20', txt: 'Merged, thanks!' }],
    noor: [{ day: 'Yesterday' }, { by: 'noor', t: '19:02', txt: 'Shared whiteboard.png from today’s session.', file: { name: 'whiteboard.png', size: '1.1 MB' } }],
  };

  const threadReplies = [
    { by: 'kai', t: '09:15', txt: 'Editing needs a clearer escape hint — show “Esc to cancel” inline.' },
    { by: 'noor', t: '09:17', txt: 'Agreed. The reply bar should show the original sender too.' },
    { by: 'ben', t: '09:22', txt: 'Will pick up the call tile.' },
  ];

  const roomMembers = ['ana', 'kai', 'ben', 'noor', 'lia', 'you'];

  return { hue, avc, ini, esc, spaces, rooms, people, timelines, threadReplies, roomMembers };
})();
