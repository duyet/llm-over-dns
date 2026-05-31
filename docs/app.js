/* ============ Interactive terminal demo ============ */
const QUERIES = [
  {
    label: "capital of france",
    q: "capital of france",
    answer: ["The capital of France is Paris."]
  },
  {
    label: "explain quantum computing",
    q: "explain quantum computing simply",
    answer: [
      "Quantum computers use qubits, which can be 0 and 1 at once",
      "(superposition). By combining many qubits they explore many",
      "possibilities in parallel — making certain problems far faster",
      "to solve than on a classical machine."
    ]
  },
  {
    label: "fibonacci in python",
    q: "fibonacci in python",
    answer: [
      "def fib(n):",
      "    a, b = 0, 1",
      "    for _ in range(n):",
      "        a, b = b, a + b",
      "    return a"
    ]
  },
  {
    label: "programming joke",
    q: "tell me a programming joke",
    answer: [
      "Why do programmers prefer dark mode?",
      "Because light attracts bugs."
    ]
  },
  {
    label: "hello in japanese",
    q: "hello in japanese",
    answer: ['"Hello" in Japanese is こんにちは (konnichiwa).']
  },
  {
    label: "speed of light",
    q: "speed of light in km/s",
    answer: ["The speed of light is approximately 299,792 km/s in a vacuum."]
  }
];

const termBody = document.getElementById("termBody");
const chipsEl = document.getElementById("chips");
let runToken = 0;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function lineCmd(q) {
  return `<span class="tl"><span class="p">$</span> <span class="cmd">dig</span> <span class="flag">@localhost -p 5353</span> <span class="q">'${q}'</span> <span class="cmd">TXT</span> <span class="flag">+short</span></span>`;
}

async function typeCmd(q, token) {
  // type the command char-by-char
  const full = `$ dig @localhost -p 5353 '${q}' TXT +short`;
  for (let i = 1; i <= full.length; i++) {
    if (token !== runToken) return;
    const shown = full.slice(0, i);
    termBody.innerHTML =
      `<span class="tl"><span class="muted">${escapeHtml(shown)}</span><span class="cursor"></span></span>`;
    await sleep(14);
  }
  termBody.innerHTML = lineCmd(q) + `<span class="cursor"></span>`;
}

function escapeHtml(s) {
  return s.replace(/[&<>']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "'": "&#39;" }[c]));
}

async function runQuery(item, token) {
  termBody.innerHTML = "";
  await typeCmd(item.q, token);
  if (token !== runToken) return;
  await sleep(420);

  // resolving state
  termBody.innerHTML =
    lineCmd(item.q) +
    `<span class="tl muted">;; resolving via llm gateway …</span><span class="cursor"></span>`;
  await sleep(720);
  if (token !== runToken) return;

  // stream answer lines
  let out = lineCmd(item.q);
  termBody.innerHTML = out + `<span class="cursor"></span>`;
  await sleep(120);

  for (let li = 0; li < item.answer.length; li++) {
    const line = item.answer[li];
    let acc = "";
    for (let i = 0; i < line.length; i++) {
      if (token !== runToken) return;
      acc += line[i];
      termBody.innerHTML =
        out +
        `<span class="tl ans">${escapeHtml(acc)}</span><span class="cursor"></span>`;
      await sleep(10);
    }
    out += `<span class="tl ans">${escapeHtml(line)}</span>`;
    termBody.innerHTML = out + `<span class="cursor"></span>`;
    await sleep(90);
  }
  termBody.innerHTML = out + `<span class="cursor"></span>`;
}

function selectChip(idx) {
  runToken++;
  const token = runToken;
  [...chipsEl.children].forEach((c, i) => c.classList.toggle("active", i === idx));
  runQuery(QUERIES[idx], token);
}

QUERIES.forEach((item, i) => {
  const c = document.createElement("button");
  c.className = "chip";
  c.textContent = item.label;
  c.addEventListener("click", () => selectChip(i));
  chipsEl.appendChild(c);
});

// kick off first query on load
window.addEventListener("load", () => selectChip(1));

/* ============ Features ============ */
const FEATURES = [
  { t: "Universal protocol", d: "DNS works on every device and network. If a box can resolve a name, it can talk to the model.", i: "globe" },
  { t: "Firewall-friendly", d: "Port 53 is almost never blocked. Reach AI even in locked-down or HTTP-restricted networks.", i: "shield" },
  { t: "High performance", d: "Async Rust on the Tokio runtime — stateless, thread-safe, production-ready concurrency.", i: "bolt" },
  { t: "Auto fallback", d: "Configure a list of models; the server fails over automatically if the primary is unavailable.", i: "swap" },
  { t: "Flexible providers", d: "Powered by AnyRouter (recommended) or OpenRouter — bring your own key and models.", i: "plug" },
  { t: "Docker ready", d: "Multi-arch images for amd64 and arm64. One command to deploy, restart policies included.", i: "box" },
  { t: "100% test coverage", d: "Full unit and integration suite with CI/CD, security audits, and vulnerability scanning.", i: "check" },
  { t: "Cross-platform", d: "Prebuilt binaries for Linux, macOS, and Windows. No runtime, no dependencies to chase.", i: "cpu" },
  { t: "Smart chunking", d: "Long answers split into ordered 255-char TXT records, then reassembled on the client.", i: "layers" }
];

const ICONS = {
  globe: '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c2.5 2.5 2.5 15 0 18M12 3c-2.5 2.5-2.5 15 0 18"/>',
  shield: '<path d="M12 3l7 3v5c0 4.5-3 7.5-7 9-4-1.5-7-4.5-7-9V6z"/>',
  bolt: '<path d="M13 2L4 14h6l-1 8 9-12h-6z"/>',
  swap: '<path d="M4 8h13l-3-3M20 16H7l3 3"/>',
  plug: '<path d="M9 2v6M15 2v6M6 8h12v3a6 6 0 0 1-12 0zM12 17v5"/>',
  box: '<path d="M3 7l9-4 9 4-9 4zM3 7v10l9 4 9-4V7M12 11v10"/>',
  check: '<circle cx="12" cy="12" r="9"/><path d="M8 12l3 3 5-6"/>',
  cpu: '<rect x="6" y="6" width="12" height="12" rx="2"/><path d="M9 2v2M15 2v2M9 20v2M15 20v2M2 9h2M2 15h2M20 9h2M20 15h2"/>',
  layers: '<path d="M12 3l9 5-9 5-9-5zM3 13l9 5 9-5"/>'
};

const featGrid = document.getElementById("featGrid");
FEATURES.forEach((f) => {
  const el = document.createElement("div");
  el.className = "feat";
  el.innerHTML =
    `<div class="feat-ico"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">${ICONS[f.i]}</svg></div>` +
    `<h3>${f.t}</h3><p>${f.d}</p>`;
  featGrid.appendChild(el);
});

/* ============ How it works flow ============ */
const NODES = [
  { n: "1", kind: "DNS CLIENT", h: "dig sends a TXT query", p: "Your question is the query name — no domain parsing, no HTTP." },
  { n: "2", kind: "RUST + TOKIO", h: "DNS server receives it", p: "A stateless async handler accepts the query on port 53 / 5353." },
  { n: "3", kind: "LLM GATEWAY", h: "Forwarded to provider", p: "AnyRouter or OpenRouter routes the prompt to a model." },
  { n: "4", kind: "TXT RECORDS", h: "Answer comes back", p: "The reply is chunked into TXT records and returned to dig." }
];
const flow = document.getElementById("flow");
const ARROW = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14M13 6l6 6-6 6"/></svg>';
NODES.forEach((nd, i) => {
  const col = document.createElement("div");
  col.className = "flow-col";
  col.innerHTML =
    `<div class="node"><div class="step-num">${nd.n}</div><div class="kind">${nd.kind}</div><h4>${nd.h}</h4><p>${nd.p}</p></div>`;
  flow.appendChild(col);
  if (i < NODES.length - 1) {
    const a = document.createElement("div");
    a.className = "flow-arrow";
    a.innerHTML = ARROW;
    flow.appendChild(a);
  }
});


/* ============ Quick start ============ */
const STEPS = [
  {
    step: "01", label: "clone & configure",
    html:
      `<span class="c"># clone the repo</span>\n` +
      `<span class="g">git</span> clone https://github.com/duyet/llm-over-dns.git\n` +
      `<span class="g">cd</span> llm-over-dns\n\n` +
      `<span class="c"># add your provider key (AnyRouter recommended)</span>\n` +
      `<span class="g">echo</span> <span class="q">"ANYROUTER_API_KEY=sk-ar-..."</span> &gt; .env`
  },
  {
    step: "02", label: "run the server",
    html:
      `<span class="c"># non-privileged dev port</span>\n` +
      `<span class="b">DNS_PORT=5353</span> <span class="g">cargo</span> run --release\n\n` +
      `<span class="c">=== Server Ready ===</span>\n` +
      `DNS server listening on <span class="b">0.0.0.0:5353</span>\n` +
      `Waiting for DNS queries...`
  },
  {
    step: "03", label: "ask anything",
    html:
      `<span class="c"># from a second terminal</span>\n` +
      `<span class="g">dig</span> <span class="b">@localhost -p 5353</span> <span class="q">'what is rust in one sentence'</span> TXT +short\n\n` +
      `<span class="q">"Rust is a systems language focused on safety,</span>\n` +
      `<span class="q"> speed, and concurrency."</span>`
  },
  {
    step: "04", label: "or run with docker",
    html:
      `<span class="g">docker</span> run -d --name llm-dns \\\n` +
      `  -p <span class="b">5353:53/udp</span> \\\n` +
      `  -e ANYROUTER_API_KEY=$KEY \\\n` +
      `  <span class="b">ghcr.io/duyet/llm-over-dns:latest</span>`
  }
];
const qsGrid = document.getElementById("qsGrid");
STEPS.forEach((s) => {
  const el = document.createElement("div");
  el.className = "code";
  el.innerHTML =
    `<div class="code-head"><span class="label"><span class="step">${s.step}</span>${s.label}</span>` +
    `<button class="copy">copy</button></div>` +
    `<pre>${s.html}</pre>`;
  const btn = el.querySelector(".copy");
  btn.addEventListener("click", () => {
    const txt = el.querySelector("pre").innerText;
    navigator.clipboard?.writeText(txt).then(() => {
      btn.textContent = "copied ✓";
      setTimeout(() => (btn.textContent = "copy"), 1400);
    });
  });
  qsGrid.appendChild(el);
});

/* ============ Use cases ============ */
const USES = [
  { n: "i", h: "Command-line AI assistant", d: "Drop a question into your shell and get an answer back as text — no app, no tab-switching.", cmd: `dig @localhost <span class="q">'what is 15% of 240'</span> TXT +short` },
  { n: "ii", h: "Restricted networks", d: "When HTTP/HTTPS is blocked but DNS resolves, you still have a way to reach a model.", cmd: `dig @ai.example.com <span class="q">'ssh connection refused help'</span> TXT` },
  { n: "iii", h: "IoT & embedded devices", d: "A DNS client is all you need — no HTTP stack, no TLS library, minimal footprint.", cmd: `dig @ai-server.local <span class="q">'analyze: 23C 45% humidity'</span> TXT` },
  { n: "iv", h: "Education & security research", d: "A vivid teaching example of creative protocol use and DNS tunneling techniques.", cmd: `dig @localhost <span class="q">'how does DNS tunneling work'</span> TXT` }
];
const ucGrid = document.getElementById("ucGrid");
USES.forEach((u) => {
  const el = document.createElement("div");
  el.className = "uc";
  el.innerHTML =
    `<div class="uc-n">${u.n}</div>` +
    `<div><h4>${u.h}</h4><p>${u.d}</p><code class="uc-cmd">${u.cmd}</code></div>`;
  ucGrid.appendChild(el);
});

/* ============ Docs links ============ */
const DOCS = [
  { h: "Getting started", d: "Detailed setup, prerequisites, and your first query.", u: "getting_started.html", i: "rocket" },
  { h: "Architecture", d: "System design, async internals, and request lifecycle.", u: "architecture.html", i: "diagram" },
  { h: "Configuration", d: "Environment variables, providers, and priority order.", u: "configuration.html", i: "gear" },
  { h: "Docker deployment", d: "Compose, multi-arch images, and production setup.", u: "deployment-docker.html", i: "box" },
  { h: "Rust API", d: "Generated API reference for the crate internals.", u: "api.html", i: "code" },
  { h: "Contributing", d: "Dev workflow, tests, lint, and PR guidelines.", u: "contributing.html", i: "heart" }
];
const DOC_ICONS = {
  rocket: '<path d="M5 13c-1.5 1.5-2 5-2 5s3.5-.5 5-2M9 11a8 8 0 0 1 9-8 8 8 0 0 1-8 9zM12 8a1.5 1.5 0 1 0 3 0 1.5 1.5 0 0 0-3 0z"/>',
  diagram: '<rect x="3" y="3" width="6" height="6" rx="1"/><rect x="15" y="15" width="6" height="6" rx="1"/><path d="M9 6h6a3 3 0 0 1 3 3v6"/>',
  gear: '<circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3M5 5l2 2M17 17l2 2M19 5l-2 2M7 17l-2 2"/>',
  box: '<path d="M3 7l9-4 9 4-9 4zM3 7v10l9 4 9-4V7M12 11v10"/>',
  code: '<path d="M9 8l-4 4 4 4M15 8l4 4-4 4"/>',
  heart: '<path d="M12 20s-7-4.5-9-9a4.5 4.5 0 0 1 9-1 4.5 4.5 0 0 1 9 1c-2 4.5-9 9-9 9z"/>'
};
const docsGrid = document.getElementById("docsGrid");
DOCS.forEach((d) => {
  const a = document.createElement("a");
  a.className = "doc";
  a.href = d.u; a.target = "_blank"; a.rel = "noopener";
  a.innerHTML =
    `<div class="doc-ico"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">${DOC_ICONS[d.i]}</svg></div>` +
    `<div><h4>${d.h} <span class="arr">↗</span></h4><p>${d.d}</p></div>`;
  docsGrid.appendChild(a);
});
