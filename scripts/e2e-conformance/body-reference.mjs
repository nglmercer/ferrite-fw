// Original-response helpers on pinned Playwright, including native binary bodies.
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const require = createRequire(import.meta.url);
const moduleName = process.env.FERRITE_PLAYWRIGHT_MODULE || 'playwright';
assert.equal(require(moduleName + '/package.json').version, '1.63.0');
const {chromium} = require(moduleName);
let hits = 0;
const fixtures = {
  '/json': Buffer.from('{"answer":42,"text":"ñ🦀"}'),
  '/empty': Buffer.alloc(0), '/binary': Buffer.from([0,255,128,65]),
  '/bad': Buffer.from('{invalid'), '/large': Buffer.alloc(1024*1024+1,120),
  '/error': Buffer.from('error response')
};
const server = createServer((request,response) => {
  hits++;
  response.writeHead(request.url === '/error' ? 500 : 200, {'Content-Type':request.url === '/' ? 'text/html' : 'application/octet-stream'});
  response.end(fixtures[request.url] || 'fixture');
});
await new Promise(resolve => server.listen(0,'127.0.0.1',resolve));
let browser;
const cases = [];
try {
  browser = await chromium.launch({executablePath:process.env.FERRITE_CHROMIUM_PATH});
  const page = await browser.newPage();
  const base = `http://127.0.0.1:${server.address().port}`;
  await page.goto(base);
  for (const [path, bytes] of Object.entries(fixtures)) {
    const responsePromise = page.waitForResponse(base + path);
    await page.evaluate(path => fetch(path).then(r=>r.arrayBuffer()), path);
    const response = await responsePromise;
    const before = hits;
    assert.deepEqual(await response.body(), bytes);
    assert.equal(await response.text(), bytes.toString('utf8'));
    if(path === '/json') assert.deepEqual(await response.json(), {answer:42,text:'ñ🦀'});
    if(path === '/bad') await assert.rejects(response.json(), SyntaxError);
    assert.equal(hits,before,'helpers must not refetch');
    cases.push({path,status:response.status(),bytes:bytes.length,text:path === '/large' ? null : await response.text(),jsonRejected:path === '/bad'});
  }
  await writeFile(new URL('body-reference.json',import.meta.url), JSON.stringify({playwright:'1.63.0',engine:'chromium',cases,note:'Ferrite intentionally differs: opt-in Chromium capture, 1 MiB per body and bounded retained history. Firefox native body capture remains unsupported.'},null,2)+'\n');
} finally {
  if(browser) await browser.close();
  await new Promise(resolve => server.close(resolve));
}
console.log('Passed six pinned original-response body profiles.');
