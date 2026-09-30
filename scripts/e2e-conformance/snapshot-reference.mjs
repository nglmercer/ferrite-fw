// Actual pinned runner observations: stable captures, update modes and negation.
import {createRequire} from 'node:module';
import {mkdtemp, writeFile, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import assert from 'node:assert/strict';
const require=createRequire(import.meta.url),moduleName=process.env.FERRITE_PLAYWRIGHT_MODULE||'playwright';
const version=require(moduleName+'/package.json').version;assert.equal(version,'1.63.0');
const testModule=require.resolve(moduleName+'/test'),browserModule=require.resolve(moduleName),cli=require.resolve(moduleName+'/cli');
const root=await mkdtemp(join(tmpdir(),'ferrite-snapshot-reference-'));
const definitions=[
 {name:'missing',body:`await expect(page).toHaveScreenshot('missing.png',{timeout:1000});`},
 {name:'mismatch',seed:true,body:`await page.evaluate(()=>document.body.style.background='blue');await expect(page).toHaveScreenshot('mismatch.png',{timeout:1000});`},
 {name:'unchanged',seed:true,body:`await expect(page).toHaveScreenshot('unchanged.png',{timeout:1000});`},
 {name:'negated-missing',body:`await expect(page).not.toHaveScreenshot('negated-missing.png',{timeout:1000});`},
 {name:'dimension-mismatch',seed:true,seedOptions:{clip:{x:0,y:0,width:40,height:40}},body:`await expect(page).toHaveScreenshot('dimension-mismatch.png',{timeout:1000});`},
 {name:'never-stable',seed:true,body:`await page.evaluate(()=>{let counter=0;const draw=()=>{counter++;document.body.style.background='rgb('+(counter&255)+','+((counter>>8)&255)+','+((counter>>16)&255)+')';requestAnimationFrame(draw)};draw()});await expect(page).toHaveScreenshot('never-stable.png',{timeout:1000,animations:'allow',threshold:0,maxDiffPixels:0});`},
 {name:'never-stable-missing',body:`await page.evaluate(()=>{let counter=0;const draw=()=>{counter++;document.body.style.background='rgb('+(counter&255)+','+((counter>>8)&255)+','+((counter>>16)&255)+')';requestAnimationFrame(draw)};draw()});await expect(page).toHaveScreenshot('never-stable-missing.png',{timeout:1000,animations:'allow',threshold:0,maxDiffPixels:0});`},
];
try {
 const engines=[];
 for(const engine of ['chromium','firefox']) {
  const observations=[];
  for(const mode of ['missing','none','changed','all']) {
   const work=join(root,engine+'-'+mode);await import('node:fs/promises').then(fs=>fs.mkdir(work));
   const eventsPath=join(work,'events.jsonl');await writeFile(eventsPath,'');
   await writeFile(join(work,'playwright.config.cjs'),`module.exports={testDir:'.',workers:1,retries:0,timeout:15000,updateSnapshots:${JSON.stringify(mode)},snapshotPathTemplate:'{snapshotDir}/{arg}{ext}',snapshotDir:${JSON.stringify(join(work,'baselines'))},reporter:[['json']]};`);
   const launch=engine==='chromium'?{executablePath:process.env.FERRITE_CHROMIUM_PATH}:{channel:'moz-firefox',executablePath:process.env.FERRITE_FIREFOX_PATH||'/usr/bin/firefox'};
   const specs=definitions.map(def=>String.raw`test(${JSON.stringify(def.name)},async({},info)=>{
    const context=await browser.newContext({viewport:{width:80,height:60}});const page=await context.newPage();
    try {await page.setContent('<style>html,body{margin:0;background:red;width:80px;height:60px}</style>');
     const baseline=info.snapshotPath(${JSON.stringify(def.name+'.png')});let before;
     ${def.seed?`fs.mkdirSync(path.dirname(baseline),{recursive:true});before=await page.screenshot(${JSON.stringify(def.seedOptions||{})});fs.writeFileSync(baseline,before);`:''}
     const start=performance.now();let outcome='passed',error;
     try {${def.body}}catch(e){outcome='caught';error=e.message.replace(/\u001b\[[0-9;]*m/g,'').split('\n').filter(Boolean).slice(0,5).join('\n');}
     const bytes=fs.existsSync(baseline)?fs.readFileSync(baseline):undefined;
     fs.appendFileSync(${JSON.stringify(eventsPath)},JSON.stringify({name:${JSON.stringify(def.name)},mode,outcome,error,durationMs:Math.round(performance.now()-start),baselineExists:!!bytes,baselineChanged:!!bytes&&!!before&&!bytes.equals(before),dimensions:bytes?[bytes.readUInt32BE(16),bytes.readUInt32BE(20)]:null,attachments:info.attachments.map(a=>({name:a.name,contentType:a.contentType,exists:a.path?fs.existsSync(a.path):!!a.body}))})+'\n');
    }finally{await context.close();}
   });`).join('\n');
   await writeFile(join(work,'snapshots.spec.cjs'),`const {test,expect}=require(${JSON.stringify(testModule)});const browserType=require(${JSON.stringify(browserModule)})[${JSON.stringify(engine)}];const fs=require('node:fs'),path=require('node:path');const mode=${JSON.stringify(mode)};let browser;test.beforeAll(async()=>{browser=await browserType.launch({...${JSON.stringify(launch)},timeout:30000})});test.afterAll(async()=>{await browser?.close()});${specs}`);
   const run=spawnSync(process.execPath,[cli,'test','--config',join(work,'playwright.config.cjs')],{cwd:work,encoding:'utf8',maxBuffer:16*1024*1024,timeout:120000});
   if(run.error)throw run.error;
   assert.equal(run.status,mode==='missing'?1:0,`${engine}/${mode}: ${run.stderr}\n${run.stdout}`);
   const report=JSON.parse(run.stdout),statuses=new Map();
   const visit=suite=>{for(const spec of suite.specs||[])for(const t of spec.tests||[])statuses.set(spec.title,t.results.map(r=>r.status));for(const child of suite.suites||[])visit(child)};report.suites.forEach(visit);
   const events=(await readFile(eventsPath,'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse);assert.equal(events.length,definitions.length,`${engine}/${mode}: ${run.stderr}\n${run.stdout}`);
   for(const event of events){
    event.statuses=statuses.get(event.name);assert.deepEqual(event.statuses,[event.name==='missing'&&mode==='missing'?'failed':'passed'],`${engine}/${mode}: ${JSON.stringify(event)}\n${run.stdout}`);
    if(event.name==='missing'){assert.equal(event.outcome,mode==='none'?'caught':'passed');assert.equal(event.baselineExists,mode!=='none');}
    if(event.name==='mismatch'||event.name==='dimension-mismatch'){assert.equal(event.outcome,['all','changed'].includes(mode)?'passed':'caught');assert.equal(event.baselineChanged,['all','changed'].includes(mode));}
    if(event.name==='negated-missing'){assert.equal(event.outcome,'caught');assert.equal(event.baselineExists,false);}
    if(event.name==='unchanged'){assert.equal(event.outcome,'passed');assert.equal(event.baselineChanged,false);}
    if(event.name==='never-stable'){assert.equal(event.outcome,['all','changed'].includes(mode)?'passed':'caught');assert.equal(event.baselineExists,true);assert.equal(event.baselineChanged,['all','changed'].includes(mode));}
    if(event.name==='never-stable-missing'){assert.equal(event.outcome,'caught');assert.equal(event.baselineExists,false);}
   }
   observations.push(...events);
  }
  engines.push({name:engine,cases:observations});
 }
 await writeFile(process.argv[2]||fileURLToPath(new URL('./snapshot-reference.json',import.meta.url)),JSON.stringify({playwright:version,engines},null,2)+'\n');
 console.log(`Recorded ${engines.reduce((n,e)=>n+e.cases.length,0)} actual snapshot cases on both engines`);
} finally {await rm(root,{recursive:true,force:true});}
