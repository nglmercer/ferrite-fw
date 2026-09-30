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
const testModule=require.resolve(modulePath+'/test'),cli=require.resolve(modulePath+'/cli');
const root=await mkdtemp(join(tmpdir(),'ferrite-ci-policy-reference-'));
const flaky=`test('recovers',async ({},info)=>{if(!info.retry)throw Error('first attempt');});`;
const cases=[
 {name:'default-flaky',body:flaky,exit:0},
 {name:'reject-flaky',body:flaky,config:{failOnFlakyTests:true},exit:1},
 {name:'cli-reject-flaky',body:flaky,args:['--fail-on-flaky-tests'],exit:1},
 {name:'ordinary-success',body:`test('pass',async()=>{});`,config:{failOnFlakyTests:true},exit:0},
 {name:'ordinary-failure',body:`test('fail',async()=>{throw Error('always');});`,config:{failOnFlakyTests:true},exit:1},
 {name:'expected-and-skipped',body:`test('expected',async()=>{test.fail();throw Error('expected');});test.skip('skip',async()=>{});test.fixme('fixme',async()=>{});`,config:{failOnFlakyTests:true},exit:0},
 {name:'expected-recovers-to-expected',body:`test('expected recovery',async ({},info)=>{test.fail();if(info.retry)throw Error('expected on retry');});`,config:{failOnFlakyTests:true},exit:1},
 {name:'repeated-flaky',body:flaky,config:{failOnFlakyTests:true,repeatEach:2},exit:1},
 {name:'flaky-does-not-consume-max-failures',body:flaky+`test('later',async()=>{});`,config:{failOnFlakyTests:true,maxFailures:1},exit:1},
 {name:'global-timeout',body:`test('hang',async()=>{await new Promise(()=>{});});`,config:{failOnFlakyTests:true,globalTimeout:1200,timeout:0},exit:1},
 {name:'focus-rejected',body:`test.only('focus',async()=>{throw Error('body entered');});`,config:{forbidOnly:true},exit:1},
 {name:'focused-skipped-suite',body:`test.describe.only('focus',()=>{test.skip('skip',async()=>{throw Error('body entered');});});`,config:{forbidOnly:true},exit:1},
 {name:'focus-filtered-out',body:`test.only('focus',async()=>{throw Error('body entered');});test('other',async()=>{});`,config:{forbidOnly:true,grep:'other'},exit:0},
 {name:'focus-in-other-shard',body:`test.only('focus',async()=>{throw Error('body entered');});`,config:{forbidOnly:true,shard:{current:2,total:2}},exit:1},
];
try {
 const observations=[];
 for(const definition of cases){
  const config={testDir:'.',workers:1,retries:1,timeout:3000,reporter:[['json'],['junit',{outputFile:join(root,'junit.xml')}]],...definition.config};
  await writeFile(join(root,'playwright.config.cjs'),`module.exports=${JSON.stringify(config)};${config.grep ? 'module.exports.grep=new RegExp(module.exports.grep);' : ''}`);
  await writeFile(join(root,'policy.spec.cjs'),`const {test}=require(${JSON.stringify(testModule)});${definition.body}`);
  const run=spawnSync(process.execPath,[cli,'test','--config',join(root,'playwright.config.cjs'),...definition.args||[]],{cwd:root,encoding:'utf8',maxBuffer:16*1024*1024,timeout:20000});
  if(run.error)throw run.error;
  if(run.status!==definition.exit)throw Error(`${definition.name}: expected exit ${definition.exit}, got ${run.status}\n${run.stderr}\n${run.stdout}`);
  let report;
  try {report=JSON.parse(run.stdout);} catch(error) {throw Error(`${definition.name}: incomplete JSON report\n${run.stderr}\n${run.stdout}`,{cause:error});}
  const tests=[];
  const visit=suite=>{for(const spec of suite.specs||[])for(const test of spec.tests||[])tests.push({title:spec.title,outcome:test.status,expectedStatus:test.expectedStatus,attempts:(test.results||[]).map(a=>({status:a.status,retry:a.retry}))});for(const child of suite.suites||[])visit(child);};
  for(const suite of report.suites)visit(suite);
  const xml=await readFile(join(root,'junit.xml'),'utf8');
  // Aggregate the actual child suites; the root may omit or use placeholder totals.
  const suites=[...xml.matchAll(/<testsuite\b[^>]*>/g)].map(match=>match[0]);
  const count=attribute=>suites.reduce((sum,suite)=>sum+Number(suite.match(new RegExp('\\b'+attribute+'="(\\d+)"'))?.[1]||0),0);
  const errors=(report.errors||[]).map(e=>(e.message||e.value||'').replace(/\u001b\[[0-9;]*m/g,'').replaceAll(root,'<reference>'));
  const junit={tests:count('tests'),failures:count('failures'),errors:count('errors'),failureElements:[...xml.matchAll(/<failure\b/g)].length,errorElements:[...xml.matchAll(/<error\b/g)].length};
  if(junit.failures!==junit.failureElements||junit.errors!==junit.errorElements)throw Error(`${definition.name}: inconsistent JUnit counts\n${xml}`);
  const expectedOutcomes={
   'default-flaky':['flaky'],'reject-flaky':['flaky'],'cli-reject-flaky':['flaky'],
   'ordinary-success':['expected'],'ordinary-failure':['unexpected'],
   'expected-and-skipped':['expected','skipped','skipped'],'expected-recovers-to-expected':['flaky'],
   'repeated-flaky':['flaky','flaky'],'flaky-does-not-consume-max-failures':['flaky','expected'],
   'global-timeout':['skipped'],'focus-rejected':[],'focused-skipped-suite':[],
   'focus-filtered-out':['expected'],'focus-in-other-shard':[],
  };
  if(JSON.stringify(tests.map(test=>test.outcome))!==JSON.stringify(expectedOutcomes[definition.name]))throw Error(`${definition.name}: unexpected outcomes ${JSON.stringify(tests)}`);
  if(['focus-rejected','focused-skipped-suite','focus-in-other-shard'].includes(definition.name)&&!errors.some(error=>error.includes("'.only'")))throw Error(`${definition.name}: missing focused inventory error`);
  observations.push({name:definition.name,config:definition.config||{},args:definition.args||[],exitCode:run.status,stats:report.stats,tests,errors,junit});
  delete observations.at(-1).stats.startTime;
  delete observations.at(-1).stats.duration;
 }
 await writeFile(process.argv[2]||fileURLToPath(new URL('./ci-policy-reference.json',import.meta.url)),JSON.stringify({playwright:version,cases:observations},null,2)+'\n');
 console.log(`Recorded ${observations.length} actual CI-policy cases from Playwright ${version}`);
}finally {await rm(root,{recursive:true,force:true});}
