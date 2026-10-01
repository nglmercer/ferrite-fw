import {createRequire} from 'node:module';
import {mkdtemp,writeFile,readFile,rm,access} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,relative} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
const require=createRequire(import.meta.url),modulePath=process.env.FERRITE_PLAYWRIGHT_MODULE||'playwright';
const version=require(modulePath+'/package.json').version;
if(version!=='1.63.0')throw Error('Reference requires Playwright 1.63.0');
const testModule=require.resolve(modulePath+'/test'),cli=require.resolve(modulePath+'/cli');
const root=await mkdtemp(join(tmpdir(),'ferrite-retention-reference-'));
const exists=async path=>{try{await access(path);return true;}catch{return false;}};
const observations=[];
try {
 for(const mode of ['always','never','failures-only'])for(const scenario of ['outcomes','interruption']) {
  const directory=join(root,mode+'-'+scenario);
  const {mkdir}=await import('node:fs/promises');await mkdir(directory);
  const journal=join(directory,'journal.jsonl');
  const config={testDir:'.',outputDir:join(directory,'output'),preserveOutput:mode,workers:scenario==='interruption'?2:1,retries:scenario==='interruption'?0:1,timeout:3000,reporter:'json',fullyParallel:scenario==='interruption',maxFailures:scenario==='interruption'?1:0};
  await writeFile(join(directory,'playwright.config.cjs'),'module.exports='+JSON.stringify(config));
  const setup=`const {test}=require(${JSON.stringify(testModule)});const fs=require('node:fs');
 test.beforeEach(async ({},info)=>{const path=info.outputPath('marker.txt');fs.writeFileSync(path,info.title);fs.appendFileSync(${JSON.stringify(journal)},JSON.stringify({name:info.title,retry:info.retry,path})+'\\n');await info.attach('marker',{path,contentType:'text/plain'});});`;
  const body=scenario==='interruption'?`
 test('interrupted',async()=>{await new Promise(()=>{});});
 test('trigger',async()=>{await new Promise(r=>setTimeout(r,500));throw Error('stop scheduling');});`:`
 test.afterEach(async ({},info)=>{if(info.title==='cleanup failure')throw Error('cleanup failure');});
 test('passing',async()=>{});
 test('failed',async()=>{throw Error('failure');});
 test('recovers',async ({},info)=>{if(info.retry===0)throw Error('first attempt');});
 test('expected failure',async()=>{test.fail();throw Error('expected');});
 test('unexpected pass',async()=>{test.fail();});
 test('timeout',async()=>{test.setTimeout(80);await new Promise(()=>{});});
 test('cleanup failure',async()=>{});
 test('runtime skip',async()=>{test.skip();});
 test.skip('registered skip',async()=>{});`;
  await writeFile(join(directory,'retention.spec.cjs'),setup+body);
  const run=spawnSync(process.execPath,[cli,'test','--config',join(directory,'playwright.config.cjs')],{cwd:directory,encoding:'utf8',maxBuffer:16*1024*1024,timeout:30000});
  if(run.error)throw run.error;
  if(run.status!==1)throw Error(`${mode}/${scenario}: expected exit 1, got ${run.status}\n${run.stderr}`);
  const report=JSON.parse(run.stdout),tests=[];
  const visit=suite=>{for(const spec of suite.specs||[])for(const test of spec.tests||[])tests.push({name:spec.title,outcome:test.status,expectedStatus:test.expectedStatus,attempts:test.results.map(result=>({retry:result.retry,status:result.status,attachments:result.attachments.map(a=>({name:a.name,path:a.path?relative(directory,a.path):null}))}))});for(const child of suite.suites||[])visit(child);};
  for(const suite of report.suites)visit(suite);
  const journalRows=(await readFile(journal,'utf8')).trim().split('\n').map(line=>JSON.parse(line));
  for(const test of tests)for(const attempt of test.attempts){
   const row=journalRows.find(row=>row.name===test.name&&row.retry===attempt.retry);
   attempt.markerCreated=!!row;attempt.markerRetained=row?await exists(row.path):false;
   for(const attachment of attempt.attachments)attachment.retained=attachment.path?await exists(join(directory,attachment.path)):false;
  }
  if(tests.length!==(scenario==='outcomes'?9:2))throw Error('Incomplete retention reference');
  if(scenario==='interruption'&&!tests.some(test=>test.attempts.some(a=>a.status==='interrupted'&&a.markerCreated)))throw Error('Missing actual interruption');
  for(const test of tests)for(const attempt of test.attempts){
   const keep=mode==='always'||(mode==='failures-only'&&attempt.status!=='skipped'&&attempt.status!==test.expectedStatus);
   if(attempt.markerCreated&&attempt.markerRetained!==keep)throw Error(`${mode}/${scenario}/${test.name}/${attempt.retry}: marker policy differs from observed classification`);
   if(attempt.attachments.some(a=>a.path&&a.retained!==keep))throw Error(`${mode}/${scenario}/${test.name}: attachment retention differs`);
  }
  tests.sort((a,b)=>a.name.localeCompare(b.name));
  observations.push({policy:mode,scenario,exitCode:run.status,tests});
 }
 await writeFile(process.argv[2]||fileURLToPath(new URL('./output-retention-reference.json',import.meta.url)),JSON.stringify({playwright:version,scope:'Filesystem-only test runner; no browser backend used',cases:observations},null,2)+'\n');
 console.log(`Recorded ${observations.length} actual Playwright output-retention runs`);
}finally{await rm(root,{recursive:true,force:true});}
