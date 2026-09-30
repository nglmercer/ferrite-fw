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
const root=await mkdtemp(join(tmpdir(),'ferrite-polling-reference-'));
const cases=[
 {name:'poll-cadence',expression:`await expect.poll(()=>{probe();return probes.length>=5?1:0;},{timeout:1000,intervals:[20,40],message:'API eventually ready'}).toBe(1);`,outcome:'passed',probes:5},
 {name:'pass-cadence',expression:`await expect(()=>{probe();expect(probes.length).toBe(5);},'block eventually completes').toPass({timeout:1000,intervals:[20,40]});`,outcome:'passed',probes:5},
 {name:'default-poll-cadence',expression:`await expect.poll(()=>{probe();return probes.length>=4?1:0;},{timeout:3000}).toBe(1);`,outcome:'passed',probes:4},
 {name:'poll-final-mismatch',expression:`await expect.poll(()=>{probe();return 0;},{timeout:150,intervals:[20,40],message:'API never ready'}).toBe(1);`,outcome:'caught'},
 {name:'empty-intervals',expression:`await expect.poll(()=>{probe();return probes.length>=3?1:0;},{timeout:200,intervals:[]}).toBe(1);`},
 {name:'zero-intervals',expression:`await expect.poll(()=>{probe();return probes.length>=5?1:0;},{timeout:200,intervals:[0]}).toBe(1);`,outcome:'passed',probes:5},
 {name:'poll-callback-error',expression:`await expect.poll(()=>{probe();if(probes.length===1)throw Error('operation failure');return 1;},{timeout:200,intervals:[20]}).toBe(1);`,outcome:'caught',probes:1},
 {name:'pass-thrown-error',expression:`await expect(()=>{probe();if(probes.length<3)throw Error('operation failure');}).toPass({timeout:200,intervals:[20]});`,outcome:'passed',probes:3},
 {name:'soft-outer-poll',expression:`await expect.configure({soft:true}).poll(()=>{probe();return 0;},{timeout:100,intervals:[20],message:'one soft failure'}).toBe(1);`,exit:1,status:'failed',outcome:'passed',softErrors:1},
 {name:'nested-soft-in-pass',expression:`await expect(()=>{probe();expect.soft(probes.length,'inner soft').toBe(3);}).toPass({timeout:150,intervals:[20]});`,exit:1,status:'failed',outcome:'passed',probes:1,softErrors:1},
 {name:'default-pass-ignores-expect-timeout',expression:`await expect.configure({timeout:10})(()=>{probe();expect(probes.length).toBe(5);}).toPass({intervals:[20]});`,outcome:'passed',probes:5},
 {name:'zero-poll-timeout',expression:`await expect.poll(()=>{probe();return probes.length>=5?1:0;},{timeout:0,intervals:[20]}).toBe(1);`,outcome:'passed',probes:5},
 {name:'hung-probe-enclosing-timeout',expression:`await expect.poll(()=>{probe();return new Promise(()=>{});},{timeout:0,intervals:[20]}).toBe(1);`,testTimeout:300,outcome:'caught',probes:1},
];
try {
 const observations=[];
 for(const definition of cases){
  const eventsPath=join(root,'events.jsonl');await writeFile(eventsPath,'');
  await writeFile(join(root,'reporter.cjs'),String.raw`const fs=require('node:fs');module.exports=class {onStepEnd(test,result,step){if(step.category==='expect')fs.appendFileSync(${JSON.stringify(eventsPath)},JSON.stringify({kind:'step',title:step.title,error:!!step.error})+'\n');}};`);
  await writeFile(join(root,'playwright.config.cjs'),`module.exports={testDir:'.',workers:1,retries:0,timeout:${definition.testTimeout||4000},reporter:[['json'],[${JSON.stringify(join(root,'reporter.cjs'))}]]};`);
  await writeFile(join(root,'polling.spec.cjs'),String.raw`const {test,expect}=require(${JSON.stringify(testModule)});const fs=require('node:fs');
  test(${JSON.stringify(definition.name)},async ({},info)=>{const start=performance.now(),probes=[];
  const event=value=>fs.appendFileSync(${JSON.stringify(eventsPath)},JSON.stringify(value)+'\n');
  const probe=()=>{probes.push(Math.round(performance.now()-start));event({kind:'probe',at:probes.at(-1)});};
  let outcome='passed',error;try{${definition.expression}}catch(e){outcome='caught';error=e.message.replace(/\u001b\[[0-9;]*m/g,'').split('\n').filter(Boolean).slice(0,3).join('\n');}
  event({kind:'result',outcome,error,softErrors:info.errors.length,durationMs:Math.round(performance.now()-start)});});`);
  const run=spawnSync(process.execPath,[cli,'test','--config',join(root,'playwright.config.cjs')],{cwd:root,encoding:'utf8',maxBuffer:16*1024*1024,timeout:20000});
  if(run.error)throw run.error;
  if(run.status!==(definition.exit||0))throw Error(`${definition.name}: unexpected exit ${run.status}\n${run.stderr}\n${run.stdout}`);
  const report=JSON.parse(run.stdout),attempts=[];
  const visit=suite=>{for(const spec of suite.specs||[])for(const test of spec.tests||[])attempts.push(...test.results);for(const child of suite.suites||[])visit(child);};
  for(const suite of report.suites)visit(suite);
  if(attempts.length!==1||attempts[0].status!==(definition.status||'passed'))throw Error(`${definition.name}: unexpected attempt outcome`);
  const events=(await readFile(eventsPath,'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse);
  const probes=events.filter(e=>e.kind==='probe').map(e=>e.at),result=events.find(e=>e.kind==='result');
  if(definition.outcome&&result?.outcome!==definition.outcome)throw Error(`${definition.name}: unexpected polling outcome ${JSON.stringify(result)}`);
  if('probes' in definition&&probes.length!==definition.probes)throw Error(`${definition.name}: unexpected probe count ${probes.length}`);
  if('softErrors' in definition&&result?.softErrors!==definition.softErrors)throw Error(`${definition.name}: unexpected soft errors`);
  observations.push({name:definition.name,exitCode:run.status,status:attempts[0].status,probeTimesMs:probes,result:result||null,assertionSteps:events.filter(e=>e.kind==='step'),attemptErrors:attempts[0].errors.length});
 }
 await writeFile(process.argv[2]||fileURLToPath(new URL('./polling-reference.json',import.meta.url)),JSON.stringify({playwright:version,cases:observations},null,2)+'\n');
 console.log(`Recorded ${observations.length} actual polling cases from Playwright ${version}`);
}finally {await rm(root,{recursive:true,force:true});}
