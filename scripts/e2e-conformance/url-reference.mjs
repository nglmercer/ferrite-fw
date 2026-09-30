import http from 'node:http';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { writeFile } from 'node:fs/promises';
const require = createRequire(import.meta.url);
const modulePath = process.env.FERRITE_PLAYWRIGHT_MODULE || 'playwright';
const { chromium } = require(modulePath);
const { expect } = require(modulePath + "/test");
const { version } = require(modulePath + '/package.json');
if (version !== '1.63.0') throw Error('Reference must use Playwright 1.63.0');
const server = http.createServer((request,response) => { response.writeHead(200,{'content-type':'text/html'}); response.end('<p>URL fixture</p>'); });
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const origin = `http://127.0.0.1:${server.address().port}`;
let browser;
const cases = [
  ['glob','**/api/item.js','/api/item.js'],
  ['glob','**/api/**/item.js','/api/item.js'],
  ['glob','**/api/**/item.js','/api/deep/nested/item.js'],
  ['glob','**/api/*.js','/api/deep/item.js'],
  ['glob','**/api/{item,other}.js','/api/other.js'],
  ['glob','**/api/{item,other}.js','/api/missing.js'],
  ['glob','/api/*.js','/api/item.js'],
  ['glob','api/*.js','/base/api/item.js'],
  ['glob','../api/*.js','/api/item.js'],
  ['glob','/api/*?active=*','/api/item?active=yes'],
  ['glob','/api/*?active=*','/api/itemXactive=yes'],
  ['glob','/literal\\*','/literal*'],
  ['glob','/literal\\*','/literal-other'],
  ['glob','$ORIGIN/API/*.js','/API/item.js'],
  ['glob','$ORIGIN/API/*.js','/api/item.js'],
  ['glob','http://*/api/item.js','/api/item.js'],
  ['glob','{http,https}://*/api/item.js','/api/item.js'],
  ['exact','/api/item.js','/api/item.js'],
  ['exact','api/item.js','/base/api/item.js'],
  ['regex','item','/api/item.js'],
  ['regex','^item$','/api/item.js'],
  ['regex','/api/item\\.js$','/api/item.js'],
];
try {
  browser = await chromium.launch({executablePath:process.env.FERRITE_CHROMIUM_PATH,headless:true});
  const context = await browser.newContext({baseURL:origin+'/base/'});
  const page = await context.newPage();
  const result = {playwright:version, engine:'chromium', browser:await browser.version(), cases:[], invalid_globs:[]};
  for (const [kind,pattern,path] of cases) {
    await page.goto(path);
    const resolved = pattern.replaceAll('$ORIGIN',origin);
    const matcher = kind==='regex' ? new RegExp(resolved) : resolved;
    let matches = false;
    try { await page.waitForURL(matcher,{timeout:70,waitUntil:'commit'}); matches=true; }
    catch(error) { if (error.name!=='TimeoutError') throw error; }
    let assertion = false;
    // Playwright assertion strings are exact; glob cases compare waits/routes.
    const assertionMatcher = matcher;
    if (kind !== 'glob') {
      try { await expect(page).toHaveURL(assertionMatcher,{timeout:70}); assertion=true; }
      catch {}
      if (assertion!==matches) throw Error('URL assertion and wait disagree: '+pattern);
    }
    let routes=0;
    await page.route(matcher,route => { routes++;return route.fulfill({status:200,contentType:'text/plain',body:'matched'}); });
    const body=await page.evaluate(async path=>fetch(path).then(r=>r.text()),path);
    await page.unroute(matcher);
    if ((body==='matched')!==matches || routes!==Number(matches)) throw Error('Route and wait disagree: '+pattern);
    result.cases.push({kind,pattern,path,matches});
  }
  for (const pattern of ['{unfinished','extra}','{nested,{brace}}']) {
    let rejected=false;
    try { await page.route(pattern,route=>route.continue()); } catch { rejected=true; }
    result.invalid_globs.push({pattern,rejected});
  }
  const output=process.argv[2] || fileURLToPath(new URL('./url-reference.json',import.meta.url));
  await writeFile(output,JSON.stringify(result,null,2)+'\n');
  console.log(`Recorded ${result.cases.length} URL cases on Playwright ${version}, Chromium ${result.browser}: ${output}`);
} finally { await browser?.close();server.close(); }
