async function refresh() {
  const r = await fetch('/cgi/hello.py?action=session');
  const t = await r.text();
  document.getElementById('sess').textContent = t.trim() || 'No session data yet.';
}
document.getElementById('bump')?.addEventListener('click', async () => {
  await fetch('/cgi/hello.py?action=bump', { method: 'POST' });
  refresh();
});
refresh();
