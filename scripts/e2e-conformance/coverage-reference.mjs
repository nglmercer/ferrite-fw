// Native coverage navigation and anonymous-script behavior on pinned Playwright.
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const require = createRequire(import.meta.url);
const moduleName = process.env.FERRITE_PLAYWRIGHT_MODULE || 'playwright';
assert.equal(require(moduleName + '/package.json').version,'1.63.0');
const {chromium} = require(moduleName);
const server = createServer((request,response) => {
  const url=request.url;
  const marker=url.includes('first') ? 'first' : 'second';
  if(url.endsWith('.js')) {response.setHeader('Content-Type','application/javascript');response.end(`function ${marker}Marker(flag){return flag ? 1 : 2}; ${marker}Marker(true);`);}
  else if(url.endsWith('.css')) {response.setHeader('Content-Type','text/css');response.end(`.${marker}{color:red}.unused-${marker}{color:blue}`);}
  else {response.setHeader('Content-Type','text/html');response.end(`<link rel=stylesheet href='/${marker}.css'><script src='/${marker}.js'></script><div class=${marker}>${marker}</div>`);}
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const base=`http://127.0.0.1:${server.address().port}`;
let browser;
const cases=[];
try {
  browser=await chromium.launch({executablePath:process.env.FERRITE_CHROMIUM_PATH});
  const page=await browser.newPage();
  for(const resetOnNavigation of [true,false]) {
    await page.coverage.startJSCoverage({resetOnNavigation});
    await page.coverage.startCSSCoverage({resetOnNavigation});
    for(const name of ['first','second']) {await page.goto(`${base}/${name}`);await page.evaluate(name=>getComputedStyle(document.querySelector(`.${name}`)).color,name);}
    const js=await page.coverage.stopJSCoverage(),css=await page.coverage.stopCSSCoverage();
    const scripts=js.map(entry=>new URL(entry.url).pathname).sort();
    const styles=css.map(entry=>new URL(entry.url).pathname).sort();
    assert.ok(scripts.includes('/second.js'));assert.ok(styles.includes('/second.css'));
    if(resetOnNavigation) {assert.ok(!scripts.includes('/first.js'));assert.ok(!styles.includes('/first.css'));}
    assert.ok(js.find(entry=>entry.url.endsWith('/second.js')).source.includes('secondMarker'));
    cases.push({resetOnNavigation,scripts,styles});
  }
  const anonymous=[];
  for(const reportAnonymousScripts of [false,true]) {
    await page.coverage.startJSCoverage({reportAnonymousScripts});
    await page.evaluate(()=>eval('window.AnonymousMarker=42'));
    const entries=await page.coverage.stopJSCoverage();
    const observed=entries.filter(entry=>entry.source?.includes('AnonymousMarker')&&!entry.url.startsWith('http'));
    assert.equal(observed.length>0,reportAnonymousScripts);
    anonymous.push({reportAnonymousScripts,urls:observed.map(entry=>entry.url)});
  }
  await writeFile(new URL('coverage-reference.json',import.meta.url),JSON.stringify({playwright:'1.63.0',engine:'chromium',cases,anonymous,note:'Reset false preserves collection metadata but cannot guarantee native ranges across navigation. Ferrite additionally offers include_source and explicit source availability/cap status; raw V8/CSS rule ranges differ from Playwright CSS disjoint used ranges.'},null,2)+'\n');
} finally {if(browser)await browser.close();await new Promise(resolve=>server.close(resolve));}
console.log('Passed pinned coverage navigation and anonymous-script profiles.');
