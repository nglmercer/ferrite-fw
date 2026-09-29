// Optional development reference runner. No Node dependency in Ferrite runtime.
import { createRequire } from 'node:module';
import { readFile, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';

const require = createRequire(import.meta.url);
const moduleName = process.env.FERRITE_PLAYWRIGHT_MODULE || 'playwright';
const { chromium } = require(moduleName);
const { expect } = require(`${moduleName}/test`);
const version = require(`${moduleName}/package.json`).version;
assert.equal(version, '1.63.0', 'reference must use pinned Playwright');
const html = await readFile(new URL('./fixture.html', import.meta.url));
const server = createServer((req, res) => {
  if (req.url === '/redirect') { res.writeHead(302, { location: '/final' }); res.end(); }
  else { res.writeHead(200, { 'content-type': 'text/html' }); res.end(html); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
let browser;
try {
  browser = await chromium.launch({ executablePath: process.env.FERRITE_CHROMIUM_PATH });
  const context = await browser.newContext({ viewport: { width: 640, height: 480 }, baseURL: base });
  const page = await context.newPage();
  await page.goto('/fixture');
  const result = { playwright: version, reference_engine: 'chromium', reference_browser_version: await browser.version(), event_samples: [] };
  const events = [['click', { clientX: 17, ctrlKey: true }], ['keydown', { key: 'Q', code: 'KeyQ' }], ['focus', {}], ['input', { data: 'x', inputType: 'insertText' }], ['pointerdown', { pointerId: 7, pointerType: 'pen' }], ['probe', {}]];
  for (const [type, init] of events) await page.locator('#event').dispatchEvent(type, init);
  await page.locator('#event').evaluate(el => el.dispatchEvent(new InputEvent('input', { bubbles: true, cancelable: true, composed: true, data: 'typed', inputType: 'insertText' })));
  result.event_samples = await page.evaluate(() => samples);
  await page.locator('#shadow-button').dispatchEvent('shadow-probe');
  await page.locator('#shadow-button').dispatchEvent('shadow-probe', { composed: false });
  result.shadow_reached = await page.evaluate(() => shadowReached);
  const check = async assertion => { try { await assertion(); return true; } catch { return false; } };
  const opts = { timeout: 100 };
  result.assertions = {
    normalized: await check(() => expect(page.locator('#text')).toHaveText('A B Hidden', opts)),
    raw_regex: await check(() => expect(page.locator('#text')).toHaveText(/\n/, opts)),
    rendered: await check(() => expect(page.locator('#text')).toHaveText('a b', { ...opts, useInnerText: true, ignoreCase: true })),
    ordered_mixed: await check(() => expect(page.locator('.items p')).toHaveText(['Alpha', /^Beta$/, 'Gamma'], opts)),
    subset: await check(() => expect(page.locator('.items p')).toContainText(['Al', /Gamma/], opts)),
    reversed_subset: await check(() => expect(page.locator('.items p')).toContainText(['Gamma', 'Alpha'], opts)),
    class_order: await check(() => expect(page.locator('#classes')).toHaveClass('two one', opts)),
    class_duplicates: await check(() => expect(page.locator('#duplicate')).toHaveClass('one one', opts)),
    class_list: await check(() => expect(page.locator('.items p')).toHaveClass(['one alpha', /two/, 'three gamma'], opts)),
    class_list_exact: await check(() => expect(page.locator('.items p')).toHaveClass(['alpha one', /two/, 'gamma three'], opts)),
    contains_class_tokens: await check(() => expect(page.locator('#classes')).toContainClass('two one', opts)),
    contains_class_list: await check(() => expect(page.locator('.items p')).toContainClass(['one', 'two beta', 'gamma'], opts)),
    values_mixed: await check(() => expect(page.locator('#values')).toHaveValues(['a', /^be/], opts)),
    indeterminate: await check(() => expect(page.locator('#checkbox')).toBeChecked({ ...opts, indeterminate: true })),
    unchecked: await check(() => expect(page.locator('#checkbox')).toBeChecked({ ...opts, checked: false })),
    viewport_low: await check(() => expect(page.locator('#clipped')).toBeInViewport({ ...opts, ratio: .4 })),
    viewport_high: await check(() => expect(page.locator('#clipped')).toBeInViewport({ ...opts, ratio: .7 })),
    accessible_name: await check(() => expect(page.locator('#accessible')).toHaveAccessibleName(/save changes/i, opts)),
    accessible_description: await check(() => expect(page.locator('#accessible')).toHaveAccessibleDescription(/helpful/i, opts)),
    accessible_error: await check(() => expect(page.locator('#accessible')).toHaveAccessibleErrorMessage(/invalid/i, opts)),
  };
  const png = await page.screenshot();
  result.screenshot = { width: png.readUInt32BE(16), height: png.readUInt32BE(20), background: await page.evaluate(() => getComputedStyle(document.documentElement).backgroundColor.match(/\d+/g).map(Number)) };
  await page.exposeFunction('double', value => value * 2);
  const redirects = [];
  page.on('response', response => { const path = new URL(response.url()).pathname; if (['/redirect', '/final'].includes(path)) redirects.push([path, response.status()]); });
  await page.goto('/redirect');
  result.redirects = redirects;
  result.callback_after_navigation = await page.evaluate(() => double(21));
  await page.goto('/api/item.js');
  result.url_matchers = [];
  for (const pattern of ['**/api/item.js', '**/api/**/item.js', '**/api/{item,other}.js', '**/api/*.css']) {
    result.url_matchers.push({ pattern, matches: await check(() => page.waitForURL(pattern, { timeout: 100 })) });
  }
  const output = process.argv[2] || fileURLToPath(new URL('./reference.json', import.meta.url));
  await writeFile(output, JSON.stringify(result, null, 2) + '\n');
  console.log(`Recorded Playwright ${version} / ${await browser.version()} reference: ${output}`);
} finally {
  await browser?.close();
  server.close();
}
