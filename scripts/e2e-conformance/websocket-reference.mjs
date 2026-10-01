// Real pinned WebSocket observations; no route/injection helpers.
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {dirname, join} from 'node:path';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const require = createRequire(import.meta.url);
const moduleName = process.env.FERRITE_PLAYWRIGHT_MODULE || 'playwright';
assert.equal(require(moduleName + '/package.json').version, '1.63.0');
const pw = require(moduleName);
const core = dirname(require.resolve(moduleName + '/package.json')) + '/../playwright-core';
const {wsServer: WebSocketServer} = require(join(core, 'lib/utilsBundle.js'));
const server = createServer((_, response) => response.end('<h1>sockets</h1>'));
const echo = new WebSocketServer({noServer: true});
let rejected;
server.on('upgrade', (request, socket, head) => {
  if (request.url === '/reject') { rejected = socket; return; }
  echo.handleUpgrade(request, socket, head, ws => {
    ws.on('message', (bytes, binary) => ws.send(bytes, {binary}));
  });
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
const url = base.replace('http:', 'ws:') + '/socket';
const engines = [];
try {
  for (const name of ['chromium', 'firefox']) {
    const launch = name === 'chromium' ? {executablePath: process.env.FERRITE_CHROMIUM_PATH} : {channel: 'moz-firefox', executablePath: process.env.FERRITE_FIREFOX_PATH || '/usr/bin/firefox'};
    const browser = await pw[name].launch({...launch, timeout: 30000});
    try {
      const page = await browser.newPage(); await page.goto(base);
      const sockets = []; page.on('websocket', socket => sockets.push(socket));
      const created = page.waitForEvent('websocket', {timeout: 3000}).then(socket => ({socket}), error => ({error}));
      await page.evaluate(url => new Promise((resolve, reject) => {
        window.sockets = [new WebSocket(url), new WebSocket(url)];
        Promise.all(sockets.map(s => new Promise((r,j) => {s.onopen=r;s.onerror=j}))).then(resolve,reject);
      }), url);
      const observed = await created;
      if (observed.error) {
        assert.equal(name, 'firefox'); assert.equal(sockets.length, 0);
        engines.push({engine: name, transport: 'stock Firefox BiDi', websocketObservation: 'unsupported', connectedSockets: 2, error: observed.error.message});
        await page.close(); continue;
      }
      assert.equal(sockets.length, 2); assert.notEqual(sockets[0], sockets[1]);
      assert.equal(sockets[0].url(), sockets[1].url());
      const textWait = sockets[0].waitForEvent('framereceived');
      await page.evaluate(() => sockets[0].send('雪<&'));
      const text = (await textWait).payload;
      assert.equal(typeof text, 'string'); assert.equal(text, '雪<&');
      const binaryWait = sockets[0].waitForEvent('framereceived');
      await page.evaluate(() => sockets[0].send(new Uint8Array([0,255,240,128,1])));
      const binary = (await binaryWait).payload;
      assert.ok(Buffer.isBuffer(binary)); assert.deepEqual([...binary], [0,255,240,128,1]);
      await assert.rejects(sockets[0].waitForEvent('framereceived', {timeout: 20}), /Timeout/);
      const closed = sockets[0].waitForEvent('close');
      const unmatched = sockets[0].waitForEvent('framereceived', {timeout: 5000}).then(() => 'unexpected', () => 'closed');
      await page.evaluate(() => sockets[0].close()); await closed;
      assert.equal(await unmatched, 'closed'); assert.equal(sockets[0].isClosed(), true);
      assert.equal(sockets[1].isClosed(), false);
      const badCreated = page.waitForEvent('websocket');
      await page.evaluate(url => {window.bad = new WebSocket(url);}, base.replace('http:', 'ws:') + '/reject');
      const bad = await badCreated;
      const errorWait = bad.waitForEvent('socketerror', {timeout: 5000});
      // Wait for the actual HTTP upgrade, bounded independently of its rejection.
      const end = Date.now() + 5000;
      while (!rejected && Date.now() < end) await new Promise(r => setTimeout(r, 5));
      assert.ok(rejected); rejected.end('HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n'); rejected = undefined;
      const error = await errorWait; assert.ok(typeof error === 'string' && error.length);
      engines.push({engine: name, sameUrlDistinctSockets: true, text, binary: [...binary], closed: sockets[0].isClosed(), unmatchedWait: 'closed', nativeError: error});
      await page.close();
    } finally { await browser.close(); }
  }
} finally {
  for (const client of echo.clients) client.terminate();
  echo.close(); server.close();
}
await writeFile(new URL('websocket-reference.json', import.meta.url), JSON.stringify({playwright: '1.63.0', engines}, null, 2) + '\n');
console.log('Passed Chromium text/binary, identity, error, timeout and close; recorded stock Firefox BiDi socket capability.');
