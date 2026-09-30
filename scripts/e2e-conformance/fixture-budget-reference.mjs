import {createRequire} from 'node:module';
import {mkdtemp, writeFile, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';

const require=createRequire(import.meta.url);
const modulePath=process.env.FERRITE_PLAYWRIGHT_MODULE||'playwright';
const version=require(modulePath+'/package.json').version;
if(version!=='1.63.0')throw Error('Reference requires Playwright 1.63.0');
const testModule=require.resolve(modulePath+'/test');
const cli=require.resolve(modulePath+'/cli');
const root=await mkdtemp(join(tmpdir(),'ferrite-fixture-budget-reference-'));
const definitions=[
 {name:'default-setup-timeout',setup:450,teardown:0,scope:'test',status:'timedOut'},
 {name:'explicit-setup-and-teardown',setup:350,teardown:350,timeout:1100,scope:'test',status:'passed'},
 {name:'explicit-limit-shared-between-setup-and-teardown',setup:350,teardown:350,timeout:600,scope:'test',status:'timedOut'},
 {name:'zero-fixture-limit',setup:350,teardown:350,timeout:0,scope:'test',status:'passed'},
 {name:'worker-default-setup-timeout',setup:450,teardown:0,scope:'worker',status:'timedOut'},
 {name:'worker-explicit-limit',setup:350,teardown:350,timeout:1100,scope:'worker',status:'passed'},
 {name:'failed-setup-releases-dependency',setup:0,teardown:0,failSetup:true,scope:'test',status:'failed'},
 {name:'teardown-timeout-dependency-accounting',setup:0,teardown:450,scope:'test',status:'timedOut'},
];
try {
 await writeFile(join(root,'playwright.config.cjs'),`module.exports={testDir:'.',workers:1,timeout:200,retries:0,reporter:'json'};`);
 const cases=[];
 for(const definition of definitions){
  const eventsPath=join(root,'events.jsonl');
  await writeFile(eventsPath,'');
  const opts={scope:definition.scope};
  if('timeout' in definition)opts.timeout=definition.timeout;
  await writeFile(join(root,'budgets.spec.cjs'),`const {test:base}=require(${JSON.stringify(testModule)});
 const fs=require('node:fs');
 const event=value=>fs.appendFileSync(${JSON.stringify(eventsPath)},JSON.stringify(value)+'\\n');
 const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
 const test=base.extend({
   dependency:[async ({},use)=>{event('dependency setup');await use(42);event('dependency teardown');},{scope:${JSON.stringify(definition.scope)}}],
   probe:[async ({dependency},use)=>{
     event('probe setup');await sleep(${definition.setup});
     ${definition.failSetup ? "throw Error('fixture setup failure');" : ''}
     event('probe ready');await use(dependency);
     event('probe teardown');await sleep(${definition.teardown});event('probe disposed');
   },${JSON.stringify(opts)}]
 });
 test(${JSON.stringify(definition.name)},async ({probe})=>{event('body');if(probe!==42)throw Error('dependency value');});`);
  const result=spawnSync(process.execPath,[cli,'test','--config',join(root,'playwright.config.cjs')],{cwd:root,encoding:'utf8',maxBuffer:16*1024*1024,timeout:20000});
  if(result.error)throw result.error;
  if(result.status!==0&&result.status!==1)throw Error(result.stderr+'\n'+result.stdout);
  const report=JSON.parse(result.stdout),attempts=[];
  const visit=suite=>{for(const spec of suite.specs||[])for(const test of spec.tests||[])attempts.push(...test.results||[]);for(const child of suite.suites||[])visit(child);};
  for(const suite of report.suites)visit(suite);
  if(attempts.length!==1)throw Error('Expected one fixture observation');
  const attempt=attempts[0];
  if(attempt.status!==definition.status)throw Error(`${definition.name}: expected ${definition.status}, got ${attempt.status}`);
  const events=(await readFile(eventsPath,'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse);
  if(!events.includes('dependency setup'))throw Error(`${definition.name}: dependency observation missing`);
  const errors=(attempt.errors||[]).map(error=>(error.message||error.value||'').replace(/\u001b\[[0-9;]*m/g,'').replaceAll(root,'<reference>'));
  cases.push({name:definition.name,options:definition,status:attempt.status,events,dependencyDisposed:events.includes('dependency teardown'),errors});
 }
 const target=process.argv[2]||fileURLToPath(new URL('./fixture-budget-reference.json',import.meta.url));
 await writeFile(target,JSON.stringify({playwright:version,testTimeoutMs:200,cases},null,2)+'\n');
 console.log(`Recorded ${cases.length} actual fixture-accounting cases from Playwright ${version}`);
}finally {await rm(root,{recursive:true,force:true});}
