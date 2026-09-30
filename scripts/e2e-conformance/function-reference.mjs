import { createRequire } from 'node:module';
import { writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
const require = createRequire(import.meta.url);
const modulePath = process.env.FERRITE_PLAYWRIGHT_MODULE || 'playwright';
const { chromium } = require(modulePath);
const { version } = require(modulePath + '/package.json');
if (version !== '1.63.0') throw Error('Reference must use Playwright 1.63.0');
const browser = await chromium.launch({executablePath:process.env.FERRITE_CHROMIUM_PATH,headless:true});
try {
  const page = await browser.newPage();
  const result = {playwright:version, engine:'chromium', browser:await browser.version(), cases:[]};
  const cases = [
    ['arg => ({received:arg})', {text:'quote " backslash \\ newline\n', nested:[1,true,null]}, 'raf'],
    ['arg => Promise.resolve(++window.calls < 3 ? false : {count:window.calls,arg})', {name:'promise'}, 30],
    ['() => [null, false, 0, "", {truthy:true}][window.calls++]', null, 'raf'],
    ['++window.calls >= 3 && {expression:true}', null, 25],
    ['() => []', null, 'raf'],
  ];
  for (const [expression,argument,polling] of cases) {
    await page.evaluate(()=>window.calls=0);
    // Playwright distinguishes a function object from an expression string.
    // Ferrite accepts Rust source strings for both, as evaluate_with_arg does.
    const predicate = expression.includes('=>') ? (0,eval)('('+expression+')') : expression;
    const handle = await page.waitForFunction(predicate,argument,{polling,timeout:2000});
    result.cases.push({expression,argument,polling,result:await handle.jsonValue()});
    await handle.dispose();
  }
  const output=process.argv[2] || fileURLToPath(new URL('./function-reference.json',import.meta.url));
  await writeFile(output,JSON.stringify(result,null,2)+'\n');
  console.log(`Recorded ${result.cases.length} function cases on Playwright ${version}, Chromium ${result.browser}: ${output}`);
} finally { await browser.close(); }
