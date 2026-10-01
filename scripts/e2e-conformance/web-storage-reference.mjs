// Pinned current-origin WebStorage snapshots; native key order is not a contract.
import {createRequire} from 'node:module';
import {writeFile} from 'node:fs/promises';
import {createServer} from 'node:http';
import assert from 'node:assert/strict';
const require = createRequire(import.meta.url);
const moduleName = process.env.FERRITE_PLAYWRIGHT_MODULE || 'playwright';
assert.equal(require(moduleName + '/package.json').version, '1.63.0');
const pw = require(moduleName);
const server = createServer((_, res) => res.end('storage'));
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const url = `http://127.0.0.1:${server.address().port}`;
const engines = [];
const normalize = entries => entries.sort((a, b) => a.name < b.name ? -1 : a.name > b.name ? 1 : 0);
try {
 for (const name of ['chromium', 'firefox']) {
  const launch = name === 'chromium' ? {executablePath: process.env.FERRITE_CHROMIUM_PATH} : {channel: 'moz-firefox', executablePath: process.env.FERRITE_FIREFOX_PATH || '/usr/bin/firefox'};
  const browser = await pw[name].launch({...launch, timeout: 30000});
  try {
   const context = await browser.newContext(), page = await context.newPage(), cases = [];
   for (const kind of ['localStorage', 'sessionStorage']) {
    await assert.rejects(page[kind].items());
    cases.push({name: `${kind}:opaque-origin`, outcome: 'error'});
   }
   await page.goto(url);
   const expected = normalize([{name: '雪', value: '😺\n"'}, {name: '', value: ''}, {name: '__proto__', value: 'safe'}, {name: 'snow', value: 'new'}]);
   for (const kind of ['localStorage', 'sessionStorage']) {
    assert.deepEqual(await page[kind].items(), []);
    for (const entry of [...expected, {name: 'snow', value: 'old'}, {name: 'snow', value: 'new'}]) await page[kind].setItem(entry.name, entry.value);
    const entries = normalize(await page[kind].items());
    assert.deepEqual(entries, expected);
    cases.push({name: `${kind}:entries-overwrite`, outcome: 'passed', entries});
   }
   const peer = await context.newPage(); await peer.goto(url);
   assert.deepEqual(normalize(await peer.localStorage.items()), expected);
   assert.deepEqual(await peer.sessionStorage.items(), []);
   cases.push({name: 'same-origin-sharing-session-isolation', outcome: 'passed'});
   await page.goto(url + '/next');
   assert.deepEqual(normalize(await page.sessionStorage.items()), expected);
   cases.push({name: 'same-origin-navigation', outcome: 'passed'});
   const state = await context.storageState();
   const captured = normalize(state.origins[0].localStorage);
   // Record prototype-sensitive capture variation without conflating it with items().
   assert.deepEqual(captured.filter(entry => entry.name !== "__proto__"), expected.filter(entry => entry.name !== "__proto__"));
   assert.ok(captured.length === expected.length || captured.length === expected.length - 1);
   const restored = await browser.newContext({storageState: state}), fresh = await restored.newPage(); await fresh.goto(url);
   assert.deepEqual(normalize(await fresh.localStorage.items()), captured);
   assert.deepEqual(await fresh.sessionStorage.items(), []);
   cases.push({name: 'storage-state-round-trip', outcome: 'passed', captured, omittedName: captured.some(entry => entry.name === '__proto__') ? null : '__proto__'});
   await page.close(); await assert.rejects(page.localStorage.items());
   cases.push({name: 'disposed-page', outcome: 'error'});
   engines.push({engine: name, cases});
  } finally {await browser.close();}
 }
 await writeFile(new URL('./web-storage-reference.json', import.meta.url), JSON.stringify({playwright: '1.63.0', engines}, null, 2) + '\n');
 console.log('Passed 16 pinned WebStorage observations across Chromium and Firefox.');
} finally {await new Promise(resolve => server.close(resolve));}
