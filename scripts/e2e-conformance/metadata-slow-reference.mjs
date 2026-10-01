// Actual pinned runner metadata inheritance and source-file slow summaries.
import {createRequire} from 'node:module';
import {mkdtemp, mkdir, writeFile, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawnSync} from 'node:child_process';
import assert from 'node:assert/strict';
const require = createRequire(import.meta.url);
const moduleName = process.env.FERRITE_PLAYWRIGHT_MODULE || 'playwright';
assert.equal(require(moduleName + '/package.json').version, '1.63.0');
const testModule = require.resolve(moduleName + '/test'), cli = require.resolve(moduleName + '/cli');
const root = await mkdtemp(join(tmpdir(), 'ferrite-metadata-reference-'));
const observations = [];
try {
 for (const [label, options] of [['default', undefined], ['disabled', null], ['top-one', {threshold: 0, max: 1}], ['zero-unlimited', {threshold: 0, max: 0}]]) {
  const dir = join(root, label); await mkdir(dir);
  const journal = join(dir, 'journal.jsonl'), captured = join(dir, 'captured.json');
  const reporter = join(dir, 'reporter.cjs');
  await writeFile(reporter, `const fs=require('node:fs');module.exports=class {onBegin(config){this.config={metadata:config.metadata,reportSlowTests:config.reportSlowTests,projects:config.projects.map(p=>({name:p.name,metadata:p.metadata}))}}onEnd(result){fs.writeFileSync(${JSON.stringify(captured)},JSON.stringify({config:this.config,status:result.status}))}}`);
  const config = {name: 'reference <&', testDir: '.', workers: 1, retries: 0, metadata: {origin: 'global', nested: [true, null, {snow: '雪<&'}]}, projects: [{name: 'inherited'}, {name: 'explicit', metadata: {origin: 'project'}}], reporter: [['list'], [reporter]], outputDir: join(dir, 'output')};
  if (options !== undefined) config.reportSlowTests = options;
  await writeFile(join(dir, 'playwright.config.cjs'), 'module.exports=' + JSON.stringify(config));
  const setup = `const {test}=require(${JSON.stringify(testModule)});const fs=require('node:fs');const body=async({},info)=>{fs.appendFileSync(${JSON.stringify(journal)},JSON.stringify({title:info.title,project:info.project.name,projectMetadata:info.project.metadata,configMetadata:info.config.metadata,retry:info.retry})+'\\n');await new Promise(r=>setTimeout(r,40))};`;
  await writeFile(join(dir, 'first.spec.cjs'), setup + `test('shared title',body);test('other title',body);`);
  await writeFile(join(dir, 'second.spec.cjs'), setup + `test('shared title',body);`);
  const result = spawnSync(process.execPath, [cli, 'test', '--config', join(dir, 'playwright.config.cjs')], {cwd: dir, encoding: 'utf8', timeout: 30000, maxBuffer: 4 * 1024 * 1024, env: {...process.env, FORCE_COLOR: '0'}});
  if (result.error) throw result.error;
  assert.equal(result.status, 0, result.stdout + result.stderr);
  const data = JSON.parse(await readFile(captured, 'utf8'));
  const tests = (await readFile(journal, 'utf8')).trim().split('\n').map(line => JSON.parse(line));
  assert.equal(tests.length, 6);
  assert.equal(data.status, 'passed');
  assert.equal(data.config.metadata.origin, 'global');
  assert.equal(data.config.projects[0].metadata.origin, 'global');
  assert.equal(data.config.projects[1].metadata.origin, 'project');
  for (const test of tests) {
   assert.equal(test.configMetadata.origin, 'global');
   assert.equal(test.projectMetadata.origin, test.project === 'inherited' ? 'global' : 'project');
  }
  const slowFiles = result.stdout.split('\n').filter(line => line.includes('Slow test file:')).map(line => line.trim());
  assert.equal(slowFiles.length, label === 'top-one' ? 1 : label === 'zero-unlimited' ? 4 : 0);
  if (label === 'default') assert.deepEqual(data.config.reportSlowTests, {max: 5, threshold: 300000});
  if (label === 'disabled') assert.equal(data.config.reportSlowTests, null);
  observations.push({mode: label, ...data, tests, slowFiles});
 }
 await writeFile(new URL('./metadata-slow-reference.json', import.meta.url), JSON.stringify({playwright: '1.63.0', observations}, null, 2) + '\n');
 console.log('Passed four pinned runs, 24 metadata observations and source-file slow-summary checks.');
} finally {await rm(root, {recursive: true, force: true});}
