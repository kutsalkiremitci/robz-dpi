// Türkçe ve İngilizce arayüz metinleri aynı anahtarlara sahip olmalı ve arayüzde kullanılan her anahtar bulunmalı.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import tr from '../ui/js/locales/tr.js';
import en from '../ui/js/locales/en.js';
import ru from '../ui/js/locales/ru.js';

const root = fileURLToPath(new URL('../ui/js/', import.meta.url));

function sources(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return entry.name === 'locales' ? [] : sources(path);
    return entry.name.endsWith('.js') ? [readFileSync(path, 'utf8')] : [];
  });
}

test('bütün dillerde aynı anahtarlar var', () => {
  assert.deepEqual(Object.keys(en).sort(), Object.keys(tr).sort());
  assert.deepEqual(Object.keys(ru).sort(), Object.keys(tr).sort());
});

test('yer tutucular bütün dillerde aynı', () => {
  const vars = (s) => [...s.matchAll(/[{]([a-zA-Z]+)[}]/g)].map((m) => m[1]).sort();
  for (const key of Object.keys(tr)) {
    for (const [name, dict] of [['en', en], ['ru', ru]]) {
      // Çoğul biçimi gereken metinler işlevdir (Rusça: 1 сайт, 2 сайта, 5 сайтов); onlar aşağıda sınanır.
      if (typeof dict[key] !== 'string') continue;
      assert.deepEqual(vars(dict[key]), vars(tr[key]), `${name}: ${key}`);
    }
  }
});

test('işlev olan metinler çalışır ve yer tutucu bırakmaz', () => {
  const samples = { n: 5, tls: 21, dns: 2, total: 3 };
  for (const [name, dict] of [['tr', tr], ['en', en], ['ru', ru]]) {
    for (const [key, value] of Object.entries(dict)) {
      if (typeof value !== 'function') continue;
      const text = value(samples);
      assert.equal(typeof text, 'string', `${name}: ${key}`);
      assert.ok(!/[{}]|undefined/.test(text), `${name}: ${key} -> ${text}`);
    }
  }
  assert.equal(ru['scope.sites']({ n: 1 }), '1 сайт');
  assert.equal(ru['scope.sites']({ n: 3 }), '3 сайта');
  assert.equal(ru['scope.sites']({ n: 11 }), '11 сайтов');
  assert.equal(ru['service.domains']({ n: 22 }), '22 адреса');
});

test("arayüzde t('…') ile kullanılan her anahtar sözlükte var", () => {
  const used = new Set();
  for (const text of sources(root)) {
    for (const m of text.matchAll(/\bt\('([a-zA-Z0-9.-]+)'/g)) used.add(m[1]);
  }
  const missing = [...used].filter((key) => !(key in tr));
  assert.deepEqual(missing, []);
  assert.ok(used.size > 50, `yalnız ${used.size} anahtar bulundu`);
});

// Kod, anahtarı çalışma anında kuruyor: t(`power.${busy}`), t(`method.${id}`), t(`problem.${code}`),
// t(`engineCode.${kod}`), t(`error.${code}`).
test('çalışma anında kurulan anahtarlar sözlükte var', () => {
  const dynamic = [
    ...['starting', 'stopping', 'applying', 'removing', 'fixing'].map((busy) => `power.${busy}`),
    ...['m1', 'm2', 'm3', 'm4'].map((id) => `method.${id}`),
    ...['nothing-selected', 'bad-dns', 'empty-strategy', 'bad-strategy'].map((code) => `problem.${code}`),
    ...['home', 'settings'].map((tab) => `tab.${tab}`),
    // Motorun çıkış kodları: WinDivert'in bilinen hataları ve engine/common.rs'teki EXIT_* sabitleri.
    ...[2, 5, 577, 654, 1275, 1753, 9001, 9002, 9003, 9004, 9005].map((code) => `engineCode.${code}`),
  ];
  assert.deepEqual(dynamic.filter((key) => !(key in tr)), []);
});

test('motorun çıkış kodlarının metni var', () => {
  const service = readFileSync(fileURLToPath(new URL('../src-tauri/src/engine/common.rs', import.meta.url)), 'utf8');
  const codes = [...service.matchAll(/pub const EXIT_[A-Z_]+: u32 = ([0-9]+);/g)].map((m) => m[1]);
  assert.ok(codes.length >= 3, `yalnız ${codes.length} sabit bulundu`);
  assert.deepEqual(codes.filter((code) => !(`engineCode.${code}` in tr)), []);
});

// Rust tarafının üretebildiği her hata kodunun bir metni olmalı; yoksa kullanıcı "Bir şeyler ters gitti" görür.
test('Rust kodundaki hata kodlarının metni var', () => {
  const rust = fileURLToPath(new URL('../src-tauri/src/', import.meta.url));
  const files = readdirSync(rust, { withFileTypes: true, recursive: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith('.rs'))
    .map((entry) => readFileSync(join(entry.parentPath, entry.name), 'utf8'));
  const codes = new Set();
  for (const text of files) {
    for (const m of text.matchAll(/DpiError::(?:new|with)\(\s*"([a-z-]+)"/g)) codes.add(m[1]);
    for (const m of text.matchAll(/io_error\("([a-z-]+)"\)/g)) codes.add(m[1]);
  }
  codes.delete('cancelled'); // yönetici onayı reddedildi: hata olarak gösterilmez
  assert.ok(codes.size > 10, `yalnız ${codes.size} kod bulundu`);
  assert.deepEqual([...codes].filter((code) => !(`error.${code}` in tr)).sort(), []);
});

// Sürüme göre farklı metinler "anahtar@linux", "anahtar@appimage", "anahtar@package" ya da "anahtar@portable" ile verilir; her birinin
// asıl anahtarı olmalı.
test('sisteme özel metinlerin asıl anahtarı var', () => {
  const variants = Object.keys(tr).filter((key) => key.includes('@'));
  assert.ok(variants.length > 5, `yalnız ${variants.length} sisteme özel metin bulundu`);
  assert.deepEqual(variants.filter((key) => !(key.split('@')[0] in tr)), []);
  assert.deepEqual(variants.filter((key) => !/@(linux|appimage|package|portable)$/.test(key)), []);
});
