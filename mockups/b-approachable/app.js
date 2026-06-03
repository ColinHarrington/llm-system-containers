/* Navigation, theme, modals, wizard, small interactions.
   Pure vanilla JS — mockup only, no real backend. */

// ---------- Screen navigation ----------
function navigate(screen, opts) {
  opts = opts || {};
  document.querySelectorAll('.screen').forEach(s => s.classList.remove('active'));
  const el = document.getElementById('screen-' + screen);
  if (el) el.classList.add('active');

  document.querySelectorAll('.nav-item').forEach(n => n.classList.toggle('active', n.dataset.screen === screen));

  // Title / breadcrumb
  const titles = {
    dashboard:   ['Home', 'Overview of your VM, sandboxes and services'],
    sandboxes:   ['Sandboxes', 'Your LLMSC workspaces (L2 system containers)'],
    'sandbox-detail': ['web-agent-01', 'Sandboxes › web-agent-01'],
    agent:       ['Agent control', 'Observe, interrupt and steer running agents'],
    services:    ['Services', 'Shared infrastructure for your sandboxes'],
    images:      ['Images', 'Base and custom sandbox images'],
    wizard:      ['Set up your environment', 'First-run configuration'],
  };
  const t = titles[screen] || ['', ''];
  document.getElementById('page-title').textContent = t[0];
  document.getElementById('page-crumb').textContent = t[1];

  document.querySelector('.main').scrollTo(0, 0);
  if (opts.tab) { /* allow deep link to a tab */ switchTab(el, opts.tab); }
}

// ---------- Theme ----------
function toggleTheme() {
  const html = document.documentElement;
  const next = html.dataset.theme === 'dark' ? 'light' : 'dark';
  html.dataset.theme = next;
  localStorage.setItem('llmsc-theme', next);
  document.getElementById('theme-ico').innerHTML = next === 'dark' ? ICON.sun : ICON.moon;
}
(function initTheme(){
  const saved = localStorage.getItem('llmsc-theme') || 'light';
  document.documentElement.dataset.theme = saved;
  window.addEventListener('DOMContentLoaded', () => {
    document.getElementById('theme-ico').innerHTML = saved === 'dark' ? ICON.sun : ICON.moon;
  });
})();

// ---------- Tabs (within a screen) ----------
function switchTab(scopeEl, name) {
  scopeEl.querySelectorAll('[data-tab]').forEach(b => b.classList.toggle('on', b.dataset.tab === name));
  scopeEl.querySelectorAll('[data-tabpanel]').forEach(p => p.style.display = (p.dataset.tabpanel === name ? '' : 'none'));
}
function tabClick(btn) {
  const scope = btn.closest('[data-tabscope]');
  switchTab(scope, btn.dataset.tab);
}

// ---------- Modals ----------
function openModal(id) { document.getElementById(id).classList.add('open'); }
function closeModal(el) {
  const m = (typeof el === 'string') ? document.getElementById(el) : el.closest('.modal-bg');
  if (m) m.classList.remove('open');
}
document.addEventListener('click', e => {
  if (e.target.classList && e.target.classList.contains('modal-bg')) e.target.classList.remove('open');
});

// ---------- VM start/stop (fake state machine) ----------
let vmState = 'running';
function toggleVm() {
  if (vmState === 'running') setVm('stopping');
  else if (vmState === 'stopped') setVm('starting');
}
function setVm(state) {
  vmState = state;
  renderVm();
  if (state === 'stopping') setTimeout(() => setVm('stopped'), 1300);
  if (state === 'starting') setTimeout(() => setVm('running'), 1800);
}
function renderVm() {
  const map = {
    running:  ['ok', 'Running', 'Stop', ICON.stop],
    stopped:  ['muted', 'Stopped', 'Start', ICON.play],
    starting: ['warn', 'Starting…', 'Starting', ICON.play],
    stopping: ['warn', 'Stopping…', 'Stopping', ICON.stop],
  };
  const [dot, label, btn, ico] = map[vmState];
  document.querySelectorAll('[data-vm-dot]').forEach(d => d.className = 'dot ' + dot + (vmState.endsWith('ing') ? ' pulse' : ''));
  document.querySelectorAll('[data-vm-label]').forEach(d => d.textContent = label);
  document.querySelectorAll('[data-vm-btn]').forEach(b => { b.innerHTML = ico + '<span>' + btn + '</span>'; b.disabled = vmState.endsWith('ing'); });
}

// ---------- Wizard ----------
let wstep = 0;
const WSTEPS = ['resources','services','networking','review'];
function gotoStep(i) {
  wstep = Math.max(0, Math.min(WSTEPS.length - 1, i));
  document.querySelectorAll('.wstep').forEach((p, idx) => p.classList.toggle('active', idx === wstep));
  document.querySelectorAll('.step').forEach((s, idx) => {
    s.classList.toggle('active', idx === wstep);
    s.classList.toggle('done', idx < wstep);
  });
  document.getElementById('wiz-back').disabled = wstep === 0;
  const next = document.getElementById('wiz-next');
  next.innerHTML = (wstep === WSTEPS.length - 1) ? (ICON.check + '<span>Create environment</span>') : '<span>Continue</span>' + ICON.arrow;
}
function wizNext() {
  if (wstep === WSTEPS.length - 1) { navigate('dashboard'); return; }
  gotoStep(wstep + 1);
}
function wizBack() { gotoStep(wstep - 1); }

// ---------- Sliders ----------
function bindSlider(id, fmt) {
  const s = document.getElementById(id);
  const out = document.getElementById(id + '-out');
  if (!s) return;
  const upd = () => out.textContent = fmt(s.value);
  s.addEventListener('input', upd); upd();
}

// ---------- Steer modal demo ----------
function sendSteer() {
  const ta = document.getElementById('steer-text');
  const v = (ta.value || '').trim();
  closeModal('modal-steer');
  if (v) {
    const log = document.getElementById('agent-log');
    if (log) {
      const div = document.createElement('div');
      div.innerHTML = '<span class="ts">12:48:31</span> <span class="agent">[operator→agent-claude]</span> steer: ' + escapeHtml(v);
      log.appendChild(div);
      log.scrollTop = log.scrollHeight;
    }
    ta.value = '';
  }
}
function escapeHtml(s){return s.replace(/[&<>]/g, c=>({'&':'&amp;','<':'&lt;','>':'&gt;'}[c]));}

// ---------- Agent pause demo ----------
let agentPaused = false;
function togglePause() {
  agentPaused = !agentPaused;
  const btn = document.getElementById('btn-pause');
  const stat = document.getElementById('agent-status-pill');
  if (agentPaused) {
    btn.innerHTML = ICON.play + '<span>Resume</span>';
    stat.className = 'pill warn'; stat.innerHTML = '<span class="dot warn"></span> Paused';
  } else {
    btn.innerHTML = ICON.pause + '<span>Pause</span>';
    stat.className = 'pill ok'; stat.innerHTML = '<span class="dot ok pulse"></span> Working';
  }
}

window.addEventListener('DOMContentLoaded', () => {
  // initial render
  gotoStep(0);
  renderVm();
  bindSlider('res-cpu', v => v + ' cores');
  bindSlider('res-mem', v => v + ' GB');
  bindSlider('res-disk', v => v + ' GB');
  // boot screen
  navigate(location.hash === '#wizard' ? 'wizard' : 'dashboard');
});

function copyChip(el) {
  const txt = el.dataset.copy || el.previousSibling.textContent;
  navigator.clipboard && navigator.clipboard.writeText(txt);
  const orig = el.innerHTML; el.textContent = 'copied';
  setTimeout(() => el.innerHTML = orig, 1100);
}
