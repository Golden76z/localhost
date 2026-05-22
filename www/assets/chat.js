const logEl = document.getElementById('log');
const statusEl = document.getElementById('status');
const form = document.getElementById('form');
const userEl = document.getElementById('user');
const textEl = document.getElementById('text');

function addLine(user, text) {
  const li = document.createElement('li');
  li.innerHTML = `<span class="who">${escapeHtml(user)}</span>${escapeHtml(text)}`;
  logEl.appendChild(li);
  logEl.scrollTop = logEl.scrollHeight;
}

function escapeHtml(s) {
  return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
}

async function loadHistory() {
  const r = await fetch('/chat/api/history');
  if (!r.ok) return;
  const msgs = await r.json();
  logEl.innerHTML = '';
  for (const m of msgs) addLine(m.user, m.text);
}

function connectSse() {
  const es = new EventSource('/chat/api/events');
  es.onopen = () => {
    statusEl.textContent = 'Connected (SSE)';
    statusEl.classList.add('live');
  };
  es.onmessage = (ev) => {
    try {
      const m = JSON.parse(ev.data);
      addLine(m.user, m.text);
    } catch (_) {}
  };
  es.onerror = () => {
    statusEl.textContent = 'Reconnecting…';
    statusEl.classList.remove('live');
    es.close();
    setTimeout(connectSse, 2000);
  };
}

form.addEventListener('submit', async (e) => {
  e.preventDefault();
  const user = userEl.value.trim() || 'guest';
  const text = textEl.value.trim();
  if (!text) return;
  const r = await fetch('/chat/api/send', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ user, text }),
  });
  if (r.ok) textEl.value = '';
});

loadHistory().then(connectSse);
