/* ============================================================
   Control Room mockup — nav, screens, SVG charts, live ticks.
   Vanilla JS, no build step.
   ============================================================ */

/* ---------- Screen routing ---------- */
function showScreen(id, push) {
  document.querySelectorAll('.screen').forEach(s => s.classList.toggle('active', s.id === 'screen-' + id));
  document.querySelectorAll('.nav-item[data-screen]').forEach(n => n.classList.toggle('active', n.dataset.screen === id));
  const titles = {
    dashboard: ['Mission Control', 'Overview'],
    sandboxes: ['Sandboxes', 'L2 system containers'],
    'sandbox-detail': ['Sandboxes', 'web-agent-01'],
    agent: ['Agent Control', 'agent-claude @ web-agent-01'],
    services: ['Services', 'Platform infrastructure'],
    images: ['Images', 'Container image catalog'],
    security: ['Security Posture', 'Network policy & eBPF'],
  };
  const t = titles[id] || ['', ''];
  document.getElementById('topTitle').textContent = t[0];
  document.getElementById('topCrumb').textContent = t[1] ? '/ ' + t[1] : '';
  document.querySelector('.content').scrollTop = 0;
  if (push !== false) location.hash = id;
}

function initNav() {
  document.querySelectorAll('[data-screen]').forEach(el => {
    el.addEventListener('click', e => {
      if (el.tagName === 'A') e.preventDefault();
      showScreen(el.dataset.screen);
    });
  });
  const initial = (location.hash || '#dashboard').slice(1);
  showScreen(initial, false);
}

/* ---------- Tab groups ---------- */
function initTabs() {
  document.querySelectorAll('[data-tabgroup]').forEach(group => {
    const g = group.dataset.tabgroup;
    group.querySelectorAll('[data-tab]').forEach(btn => {
      btn.addEventListener('click', () => {
        group.querySelectorAll('[data-tab]').forEach(b => b.classList.toggle('active', b === btn));
        document.querySelectorAll('[data-tabpane="' + g + '"]').forEach(p => {
          p.style.display = p.dataset.tabid === btn.dataset.tab ? '' : 'none';
        });
      });
    });
  });
}

/* ---------- Toggles ---------- */
function initToggles() {
  document.querySelectorAll('.toggle').forEach(t => {
    t.addEventListener('click', () => t.classList.toggle('on'));
  });
}

/* ---------- SVG chart helpers ---------- */
function rng(seed) { let s = seed; return () => { s = (s * 9301 + 49297) % 233280; return s / 233280; }; }

function sparkline(values, opts) {
  opts = opts || {};
  const w = opts.w || 120, h = opts.h || 32, pad = 2;
  const color = opts.color || '#4da3ff';
  const max = Math.max(...values), min = Math.min(...values);
  const span = (max - min) || 1;
  const step = (w - pad * 2) / (values.length - 1);
  const pts = values.map((v, i) => [pad + i * step, h - pad - ((v - min) / span) * (h - pad * 2)]);
  const d = pts.map((p, i) => (i ? 'L' : 'M') + p[0].toFixed(1) + ' ' + p[1].toFixed(1)).join(' ');
  const area = d + ` L ${pts[pts.length-1][0].toFixed(1)} ${h} L ${pts[0][0].toFixed(1)} ${h} Z`;
  const gid = 'g' + Math.random().toString(36).slice(2, 8);
  return `<svg viewBox="0 0 ${w} ${h}" width="${w}" height="${h}" preserveAspectRatio="none" style="display:block">
    <defs><linearGradient id="${gid}" x1="0" x2="0" y1="0" y2="1">
      <stop offset="0" stop-color="${color}" stop-opacity=".28"/><stop offset="1" stop-color="${color}" stop-opacity="0"/>
    </linearGradient></defs>
    <path d="${area}" fill="url(#${gid})"/>
    <path d="${d}" fill="none" stroke="${color}" stroke-width="1.6" stroke-linejoin="round" stroke-linecap="round"/>
  </svg>`;
}

function areaChart(series, opts) {
  // series: [{values, color}], opts: {w,h, grid}
  opts = opts || {};
  const w = opts.w || 600, h = opts.h || 160, padL = 30, padB = 18, padT = 8, padR = 6;
  const iw = w - padL - padR, ih = h - padB - padT;
  let all = []; series.forEach(s => all = all.concat(s.values));
  const max = opts.max != null ? opts.max : Math.max(...all) * 1.15;
  const min = 0;
  const span = (max - min) || 1;
  const n = series[0].values.length;
  const step = iw / (n - 1);
  let grid = '';
  for (let i = 0; i <= 4; i++) {
    const y = padT + (ih / 4) * i;
    const val = (max - (span / 4) * i);
    grid += `<line x1="${padL}" x2="${w-padR}" y1="${y}" y2="${y}" stroke="#18223300" stroke-width="1"/>
             <line x1="${padL}" x2="${w-padR}" y1="${y}" y2="${y}" stroke="#1a2435" stroke-width="1"/>
             <text x="${padL-6}" y="${y+3}" text-anchor="end" font-size="9" fill="#455066" font-family="monospace">${Math.round(val)}</text>`;
  }
  let paths = '';
  series.forEach((s, si) => {
    const pts = s.values.map((v, i) => [padL + i * step, padT + ih - ((v - min) / span) * ih]);
    const d = pts.map((p, i) => (i ? 'L' : 'M') + p[0].toFixed(1) + ' ' + p[1].toFixed(1)).join(' ');
    const area = d + ` L ${pts[pts.length-1][0].toFixed(1)} ${padT+ih} L ${pts[0][0].toFixed(1)} ${padT+ih} Z`;
    const gid = 'a' + si + Math.random().toString(36).slice(2, 6);
    paths += `<defs><linearGradient id="${gid}" x1="0" x2="0" y1="0" y2="1">
        <stop offset="0" stop-color="${s.color}" stop-opacity=".22"/><stop offset="1" stop-color="${s.color}" stop-opacity="0"/></linearGradient></defs>
      ${s.fill !== false ? `<path d="${area}" fill="url(#${gid})"/>` : ''}
      <path d="${d}" fill="none" stroke="${s.color}" stroke-width="1.8" stroke-linejoin="round"/>`;
  });
  return `<svg viewBox="0 0 ${w} ${h}" width="100%" height="${h}" preserveAspectRatio="none">${grid}${paths}</svg>`;
}

function donut(pct, opts) {
  opts = opts || {};
  const sz = opts.sz || 96, sw = opts.sw || 10, r = (sz - sw) / 2, c = sz / 2;
  const circ = 2 * Math.PI * r;
  const color = opts.color || '#4da3ff';
  return `<svg viewBox="0 0 ${sz} ${sz}" width="${sz}" height="${sz}">
    <circle cx="${c}" cy="${c}" r="${r}" fill="none" stroke="#1a2435" stroke-width="${sw}"/>
    <circle cx="${c}" cy="${c}" r="${r}" fill="none" stroke="${color}" stroke-width="${sw}"
      stroke-linecap="round" stroke-dasharray="${circ}" stroke-dashoffset="${circ * (1 - pct/100)}"
      transform="rotate(-90 ${c} ${c})"/>
    <text x="${c}" y="${c-1}" text-anchor="middle" font-size="18" font-weight="700" fill="#e9eef7">${pct}<tspan font-size="10" fill="#6b7a93">%</tspan></text>
    ${opts.label ? `<text x="${c}" y="${c+15}" text-anchor="middle" font-size="9" fill="#6b7a93" font-family="monospace">${opts.label}</text>` : ''}
  </svg>`;
}

function bars(values, opts) {
  opts = opts || {};
  const w = opts.w || 200, h = opts.h || 40, gap = 2;
  const color = opts.color || '#2ee6d6';
  const max = Math.max(...values) || 1;
  const bw = (w - gap * (values.length - 1)) / values.length;
  return `<svg viewBox="0 0 ${w} ${h}" width="${w}" height="${h}">` +
    values.map((v, i) => {
      const bh = Math.max(1, (v / max) * h);
      return `<rect x="${(i*(bw+gap)).toFixed(1)}" y="${(h-bh).toFixed(1)}" width="${bw.toFixed(1)}" height="${bh.toFixed(1)}" rx="1" fill="${color}" opacity="${0.45+0.55*(v/max)}"/>`;
    }).join('') + '</svg>';
}

function seedSeries(seed, n, base, amp) {
  const r = rng(seed); const out = []; let v = base;
  for (let i = 0; i < n; i++) { v += (r() - 0.48) * amp; v = Math.max(base*0.3, Math.min(base*1.8, v)); out.push(v); }
  return out;
}

/* ---------- Live ticking (sparkline updates + feed) ---------- */
let liveTimers = [];
function startLive() {
  // animated mini sparklines
  document.querySelectorAll('[data-live-spark]').forEach(el => {
    const color = el.dataset.color || '#4da3ff';
    let data = seedSeries(parseInt(el.dataset.seed || '7'), 40, parseFloat(el.dataset.base || '50'), parseFloat(el.dataset.amp || '20'));
    el.innerHTML = sparkline(data, { w: parseInt(el.dataset.w || 120), h: parseInt(el.dataset.h || 32), color });
    const t = setInterval(() => {
      data.shift(); data.push(Math.max(2, data[data.length-1] + (Math.random()-0.48) * parseFloat(el.dataset.amp || '20')));
      el.innerHTML = sparkline(data, { w: parseInt(el.dataset.w || 120), h: parseInt(el.dataset.h || 32), color });
    }, 1600);
    liveTimers.push(t);
  });
}

/* clock */
function startClock() {
  const el = document.getElementById('clock');
  if (!el) return;
  const tick = () => {
    const d = new Date();
    el.textContent = d.toLocaleTimeString('en-US', { hour12: false }) + ' UTC';
  };
  tick(); setInterval(tick, 1000);
}

/* agent live trace feed (cockpit) */
const TRACE_MSGS = [
  ['llm.call', 'POST /chat/completions → litellm · model=claude-sonnet · vkey=sk-vk-…a91'],
  ['tool', 'bash → "pytest -q tests/" (exit 0, 4.2s)'],
  ['fs', 'write /home/agent-claude/work/api/handlers.py (+38 −4)'],
  ['l3', 'docker compose up -d (postgres, redis) — rootless'],
  ['llm.call', 'POST /chat/completions → litellm · 1,204 in / 612 out tok'],
  ['net', 'egress api.github.com:443 via mitmproxy — allowed'],
  ['tool', 'bash → "git commit -m \'fix: null guard\'"'],
  ['policy', 'Tetragon: file open /etc/shadow DENIED (uid 1001)'],
  ['llm.call', 'POST /chat/completions → litellm · streaming…'],
];
function startAgentFeed() {
  const feed = document.getElementById('agentFeed');
  if (!feed) return;
  let i = 0;
  const t = setInterval(() => {
    const m = TRACE_MSGS[i % TRACE_MSGS.length]; i++;
    const now = new Date().toLocaleTimeString('en-US', { hour12: false });
    const kindColor = { 'llm.call': '#9b8cff', tool: '#2ee6d6', fs: '#4da3ff', l3: '#f5b94d', net: '#36d399', policy: '#ff5d6c' }[m[0]] || '#aab6cb';
    const line = document.createElement('div');
    line.className = 'line';
    line.innerHTML = `<span class="t">${now}</span><span style="color:${kindColor};flex:none;width:62px;font-weight:600">${m[0]}</span><span class="msg">${m[1]}</span>`;
    feed.prepend(line);
    while (feed.children.length > 40) feed.removeChild(feed.lastChild);
  }, 2200);
  liveTimers.push(t);
}

/* token counter tick */
function startTokenCounter() {
  const el = document.getElementById('tokCounter');
  if (!el) return;
  let v = 184230;
  const t = setInterval(() => {
    v += Math.floor(Math.random() * 380);
    el.textContent = v.toLocaleString();
  }, 2200);
  liveTimers.push(t);
}

/* ---------- Render static charts once DOM is ready ---------- */
function renderCharts() {
  document.querySelectorAll('[data-chart]').forEach(el => {
    const type = el.dataset.chart;
    if (type === 'area') {
      const seed = parseInt(el.dataset.seed || '3');
      const n = parseInt(el.dataset.n || '48');
      const s1 = seedSeries(seed, n, parseFloat(el.dataset.base || '55'), parseFloat(el.dataset.amp || '18'));
      const series = [{ values: s1, color: el.dataset.color || '#4da3ff' }];
      if (el.dataset.color2) series.push({ values: seedSeries(seed + 11, n, parseFloat(el.dataset.base2 || '30'), 14), color: el.dataset.color2 });
      el.innerHTML = areaChart(series, { h: parseInt(el.dataset.h || 160), max: el.dataset.max ? parseFloat(el.dataset.max) : null });
    } else if (type === 'donut') {
      el.innerHTML = donut(parseInt(el.dataset.pct), { color: el.dataset.color, label: el.dataset.label, sz: parseInt(el.dataset.sz || 96) });
    } else if (type === 'bars') {
      el.innerHTML = bars(seedSeries(parseInt(el.dataset.seed || 5), parseInt(el.dataset.n || 24), 50, 30), { w: parseInt(el.dataset.w || 200), h: parseInt(el.dataset.h || 40), color: el.dataset.color });
    } else if (type === 'spark') {
      el.innerHTML = sparkline(seedSeries(parseInt(el.dataset.seed || 5), 30, parseFloat(el.dataset.base || 50), parseFloat(el.dataset.amp || 18)), { w: parseInt(el.dataset.w || 120), h: parseInt(el.dataset.h || 32), color: el.dataset.color });
    }
  });
}

/* ---------- Boot ---------- */
document.addEventListener('DOMContentLoaded', () => {
  initNav();
  initTabs();
  initToggles();
  renderCharts();
  startLive();
  startClock();
  startAgentFeed();
  startTokenCounter();
  // VM start/stop demo toggle
  const vmBtn = document.getElementById('vmToggle');
  if (vmBtn) vmBtn.addEventListener('click', () => {
    document.querySelectorAll('[data-vmstate]').forEach(e => e.classList.toggle('hidden-x'));
  });
});

/* steer modal mock */
function openSteer() { document.getElementById('steerModal').style.display = 'flex'; }
function closeSteer() { document.getElementById('steerModal').style.display = 'none'; }
