// Geliştirme için sahte çekirdek. Yalnız arayüz Tauri dışında, düz bir tarayıcıda açıldığında yüklenir
// (bkz. bridge.js); kurulu uygulamada hiç çalışmaz. Bilgisayara dokunmaz: motoru açmış gibi yapar.
//
// Adres çubuğundan başlangıç durumu seçilebilir:
//   ?lang=en            İngilizce
//   ?on                 motor açık
//   ?tab=settings       Ayarlar sekmesi
//   ?advanced           gelişmiş bölüm açık
//   ?wizard             ilk açılış adımları (isteğe bağlı ?step=2 ya da ?step=3)
//   ?conflict           başka bir DPI aracı çalışıyor
//   ?error=<kod>        hata kutusu (örnek: engine-stopped); ?detail=1275 ile motorun çıkış kodu
const query = new URLSearchParams(location.search);
const listeners = new Set();
const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const base = {
  split: 2,
  splitAtSni: false,
  reverse: true,
  httpSplit: 2,
  fakeTtlMode: 'fixed',
  fakeTtl: 5,
  fakeBadChecksum: false,
  fakeBadSeq: false,
  fakeRepeats: 1,
  fakeHost: 'www.w3.org',
  blockQuic: false,
  noSni: 'known',
};

const catalog = {
  services: [
    { id: 'discord', name: 'Discord', domains: Array.from({ length: 22 }, (_, i) => `d${i}.example`) },
    { id: 'roblox', name: 'Roblox', domains: Array.from({ length: 8 }, (_, i) => `r${i}.example`) },
  ],
  methods: [
    { id: 'm1', flags: '-5 --set-ttl 5', strategy: base },
    { id: 'm2', flags: '-5', strategy: { ...base, fakeTtlMode: 'auto' } },
    { id: 'm3', flags: '--set-ttl 3', strategy: { ...base, split: 0, httpSplit: 0, reverse: false, fakeTtl: 3 } },
    { id: 'm4', flags: '-9', strategy: { ...base, fakeTtlMode: 'off', fakeBadChecksum: true, fakeBadSeq: true, blockQuic: true } },
  ],
  dns: [
    { id: 'quad9', name: 'Quad9', address: '9.9.9.9', port: 9953 },
    { id: 'yandex', name: 'Yandex', address: '77.88.8.8', port: 1253 },
    { id: 'adguard', name: 'AdGuard', address: '94.140.14.140', port: 5353 },
    { id: 'opendns', name: 'OpenDNS', address: '208.67.222.222', port: 5353 },
  ],
};

const settings = {
  language: query.get('lang') || 'tr',
  closeToTray: true,
  launchAtLogin: false,
  enabled: query.has('on'),
  onboarded: !query.has('wizard'),
  scope: 'selected',
  services: ['discord', 'roblox'],
  customDomains: query.has('sites') ? ['violabot.com', 'example.org'] : [],
  disabledDomains: query.has('sites') ? ['example.org'] : [],
  excludedDomains: query.has('sites') ? ['online.bank.example', 'kampus.edu.tr'] : [],
  method: 'm1',
  custom: { ...base },
  advanced: query.has('advanced'),
  dnsEnabled: true,
  dnsAll: false,
  dnsProvider: 'quad9',
  dnsCustomAddr: '',
  dnsCustomPort: 53,
};

const strategy = () => catalog.methods.find((m) => m.id === settings.method)?.strategy ?? settings.custom;
const fingerprint = () =>
  JSON.stringify([
    settings.scope,
    settings.services,
    settings.customDomains.filter((d) => !settings.disabledDomains.includes(d)),
    settings.scope === 'all' ? settings.excludedDomains : [],
    strategy(),
    settings.dnsEnabled,
    settings.dnsAll,
    settings.dnsProvider,
    settings.dnsCustomAddr,
    settings.dnsCustomPort,
  ]);

const engine = {
  installed: query.has('on'),
  state: query.has('on') ? 'running' : 'stopped',
  busy: null,
  applied: query.has('on') ? fingerprint() : null,
  error: query.get('error') ? { code: query.get('error'), detail: query.get('detail') } : null,
  startedAt: Date.now(),
};
let conflicts = query.has('conflict') ? [{ name: 'goodbyedpi.exe', pid: 4408 }] : [];
const probe = { running: false, results: [] };

function problem() {
  const enabled = settings.customDomains.filter((d) => !settings.disabledDomains.includes(d));
  if (settings.scope === 'selected' && !settings.services.length && !enabled.length) return 'nothing-selected';
  if (settings.dnsEnabled && settings.dnsProvider === 'custom' && !settings.dnsCustomAddr) return 'bad-dns';
  const s = strategy();
  if (!s.split && !s.splitAtSni && s.fakeTtlMode === 'off' && !s.fakeBadChecksum && !s.fakeBadSeq) return 'empty-strategy';
  return null;
}

// Motor açıkken sayaçlar yavaşça artar.
function stats() {
  if (engine.state !== 'running') return null;
  const ticks = Math.floor((Date.now() - engine.startedAt) / 4000);
  return {
    tls: 14 + ticks,
    http: 0,
    dns: 6 + Math.floor(ticks / 2),
    quic: strategy().blockQuic ? 2 : 0,
    known: 9,
    recent: ['gateway.discord.gg', 'cdn.discordapp.com', 'discord.com', 'apis.roblox.com', 'tr.rbxcdn.com'],
    since: Math.floor(engine.startedAt / 1000),
  };
}

function snapshot() {
  const matches = engine.installed && engine.applied === fingerprint();
  return structuredClone({
    version: '0.1.0',
    packaged: true,
    systemLanguage: 'tr',
    platform: query.get('platform') || 'windows', // ?platform=linux
    portable: query.has('portable'), // ?portable
    appImage: query.has('appimage'), // ?platform=linux&appimage
    systemPackage: query.has('package'), // ?platform=linux&package
    supported: true,
    engineReady: true,
    settings,
    catalog,
    links: {
      github: 'https://github.com/kutsalkiremitci/robz-dpi',
      discord: 'https://discord.gg/UyyEn7SHm4',
      disclaimer: 'https://github.com/kutsalkiremitci/robz-dpi/blob/main/DISCLAIMER.md',
    },
    engine: {
      installed: engine.installed,
      state: engine.state,
      busy: engine.busy,
      pending: engine.state === 'running' && !problem() && !matches,
      needsAdmin: !matches,
      problem: problem(),
      error: engine.error,
      stats: stats(),
    },
    conflicts,
    probe,
  });
}

function emit() {
  const payload = snapshot();
  for (const listener of listeners) listener({ payload });
}

async function operate(busy, work) {
  engine.error = null;
  engine.busy = busy;
  emit();
  await wait(900);
  work();
  engine.busy = null;
  emit();
}

function normalize(text) {
  const host = text.trim().toLowerCase().replace(/^[a-z]+:[/][/]/, '').replace(/[/?#].*$/, '').replace(/^www[.]/, '');
  return /^([a-z0-9-]+[.])+[a-z][a-z0-9-]*$/.test(host) ? host : null;
}

const LIMITS = { split: [0, 1200], httpSplit: [0, 1200], fakeTtl: [1, 64], fakeRepeats: [1, 10] };

const methods = {
  getState: () => snapshot(),
  turnOn: () => {
    // Gerçek çekirdekteki gibi: ilk açılış adımları bitmeden motor açılmaz.
    if (!settings.onboarded) {
      engine.error = { code: 'onboarding-required', detail: null };
      return emit();
    }
    if (problem()) {
      engine.error = { code: problem(), detail: null };
      return emit();
    }
    return operate('starting', () => {
      Object.assign(engine, { installed: true, state: 'running', applied: fingerprint(), startedAt: Date.now() });
      settings.enabled = true;
    });
  },
  turnOff: () =>
    operate('stopping', () => {
      engine.state = 'stopped';
      settings.enabled = false;
    }),
  apply: () => operate('applying', () => (engine.applied = fingerprint())),
  removeAll: () => operate('removing', () => Object.assign(engine, { installed: false, state: 'stopped', applied: null })),
  uninstallApp: async () => {
    await operate('removing', () => Object.assign(engine, { installed: false, state: 'stopped', applied: null }));
    return true;
  },
  stopConflicts: () => operate('fixing', () => (conflicts = [])),
  dismissError: () => {
    engine.error = null;
    emit();
  },
  setSetting: (key, value) => {
    if (!(key in settings) || key === 'custom') throw { code: 'bad-request', detail: key };
    settings[key] = value;
    emit();
  },
  setCustom: (field, value) => {
    if (!(field in base)) throw { code: 'bad-request', detail: field };
    const limit = LIMITS[field];
    if (limit && (!Number.isInteger(value) || value < limit[0] || value > limit[1])) throw { code: 'bad-request', detail: field };
    if (field === 'fakeHost' && !/^([a-z0-9-]+[.])+[a-z][a-z0-9-]*$/.test(value)) throw { code: 'bad-request', detail: field };
    settings.custom = { ...strategy(), [field]: value };
    settings.method = 'custom';
    emit();
  },
  setService: (id, on) => {
    settings.services = catalog.services.map((s) => s.id).filter((s) => (s === id ? on : settings.services.includes(s)));
    emit();
  },
  addDomains: (text) => {
    const added = [];
    const rejected = [];
    for (const part of text.split(/[\s,;]+/).filter(Boolean)) {
      const domain = normalize(part);
      if (!domain) rejected.push(part);
      else if (!settings.customDomains.includes(domain)) {
        settings.customDomains.push(domain);
        added.push(domain);
      }
    }
    emit();
    return { added, rejected };
  },
  removeDomain: (domain) => {
    settings.customDomains = settings.customDomains.filter((d) => d !== domain);
    settings.disabledDomains = settings.disabledDomains.filter((d) => d !== domain);
    emit();
  },
  setDomain: (domain, on) => {
    settings.disabledDomains = settings.disabledDomains.filter((d) => d !== domain);
    if (!on) settings.disabledDomains.push(domain);
    emit();
  },
  addExcluded: (text) => {
    const added = [];
    const rejected = [];
    for (const part of text.split(/[\s,;]+/).filter(Boolean)) {
      const domain = normalize(part);
      if (!domain) rejected.push(part);
      else if (!settings.excludedDomains.includes(domain)) {
        settings.excludedDomains.push(domain);
        added.push(domain);
      }
    }
    emit();
    return { added, rejected };
  },
  removeExcluded: (domain) => {
    settings.excludedDomains = settings.excludedDomains.filter((d) => d !== domain);
    emit();
  },
  test: async () => {
    probe.running = true;
    emit();
    await wait(1200);
    const on = engine.state === 'running';
    const names = [
      ...catalog.services.filter((s) => settings.scope === 'all' || settings.services.includes(s.id)).map((s) => [s.id, s.name]),
      ...settings.customDomains.map((d) => [d, d]),
    ];
    probe.results = names.map(([id, label], i) => ({ id, label, ok: on, ms: on ? 140 + i * 37 : 6000 }));
    probe.running = false;
    emit();
  },
  getLog: () =>
    [
      '2026-11-22 19:40:01  Robz DPI motoru 0.1.0 başlıyor',
      '2026-11-22 19:40:01  süzgeç: !impostor and !loopback and ((outbound and tcp and tcp.DstPort == 443 and tcp.PayloadLength > 5 and tcp.Payload[0] == 0x16 and tcp.Payload[1] == 0x03 and tcp.Payload[5] == 0x01) or (outbound and udp and udp.DstPort == 53) or (inbound and udp and (udp.SrcPort == 9953)))',
      '2026-11-22 19:40:01  çalışıyor: 30 alan adı, DNS 9.9.9.9:9953 (yalnız hedefler)',
    ].join('\n'),
  openExternal: (url) => window.open(url, '_blank', 'noopener'),
};

// Sayaçlar değiştikçe arayüz yenilensin (gerçek çekirdek 2 saniyede bir durumu yeniden okur).
setInterval(() => engine.state === 'running' && emit(), 4000);

window.__TAURI__ = {
  core: {
    async invoke(_command, { method, args }) {
      if (!methods[method]) throw { code: 'bad-request', detail: method };
      return (await methods[method](...args)) ?? null;
    },
  },
  event: {
    async listen(_name, callback) {
      listeners.add(callback);
      return () => listeners.delete(callback);
    },
  },
};

if (query.get('tab') === 'settings') {
  // Arayüz çizilir çizilmez Ayarlar sekmesine geç (modüllerin yüklenme süresi değişebilir).
  const timer = setInterval(() => {
    const tab = document.querySelectorAll('.tab')[1];
    if (!tab) return;
    clearInterval(timer);
    tab.click();
  }, 30);
}
