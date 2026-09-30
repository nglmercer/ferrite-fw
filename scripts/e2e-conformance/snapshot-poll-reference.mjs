// Actual public toPass/screenshot attachment observations, including caught failures.
import {createRequire} from 'node:module';
import {mkdtemp,writeFile,readFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import assert from 'node:assert/strict';
const require=createRequire(import.meta.url),moduleName=process.env.FERRITE_PLAYWRIGHT_MODULE||'playwright';
assert.equal(require(moduleName+'/package.json').version,'1.63.0');
const root=await mkdtemp(join(tmpdir(),'ferrite-snapshot-poll-reference-'));
function defineTests(settings){
 const {test,expect}=require(settings.testModule),pw=require(settings.browserModule),fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),assert=require('node:assert/strict');
 for(const engine of ['chromium','firefox'])test(engine,async({},info)=>{
  const launch=engine==='chromium'?{executablePath:settings.chromium}:{channel:'moz-firefox',executablePath:settings.firefox};
  const browser=await pw[engine].launch({...launch,timeout:30000}),cases=[];
  try{
   for(const scenario of ['stable-failure','eventual-success','nested-failure','nested-success','changing-failure','missing-target-failure']){
    const context=await browser.newContext({viewport:{width:40,height:30}}),page=await context.newPage(),prefix=engine+'-'+scenario;
    let observation,calls=0;const probes=[];
    const imageAttachments=()=>info.attachments.filter(a=>a.name.startsWith(prefix+'-'));
    try{
     await page.setContent('<style>html,body{height:100%;margin:0;background:red}</style>');
     const name=prefix+'.png',baseline=info.snapshotPath(name,{kind:'screenshot'}),before=await page.screenshot();
     fs.mkdirSync(path.dirname(baseline),{recursive:true});fs.writeFileSync(baseline,before);
     await page.evaluate(()=>document.body.style.background='blue');
     if(scenario==='changing-failure')await page.evaluate(()=>{let counter=0;const draw=()=>{counter++;document.body.style.background='rgb('+(counter&255)+','+((counter>>8)&255)+','+((counter>>16)&255)+')';requestAnimationFrame(draw)};draw()});
     const start=performance.now();let error;
     const snapshot=async()=>{
      const call=calls++;let outcome='passed';
      try{
       if(scenario==='eventual-success')await page.evaluate(red=>document.body.style.background=red?'red':'blue',call>0);
       await expect(scenario==='missing-target-failure'?page.locator('#missing'):page).toHaveScreenshot(name,{timeout:250,threshold:0,maxDiffPixels:0,animations:'allow'});
      }catch(caught){outcome='caught';throw caught}
      finally{probes.push({call,outcome,attachmentCount:imageAttachments().length})}
     };
     try{
      if(scenario.startsWith('nested'))await expect(async()=>{
       if(scenario==='nested-success'){try{await expect(snapshot).toPass({timeout:550,intervals:[50]})}catch{}}
       else await expect(snapshot).toPass({timeout:550,intervals:[50]});
      }).toPass({timeout:1400,intervals:[50]});
      else await expect(snapshot).toPass({timeout:1000,intervals:[50]});
     }catch(caught){error=caught.message.replace(/\u001b\[[0-9;]*m/g,'')}
     assert.equal(!!error,!scenario.endsWith('success'));assert.ok(calls>0);assert.ok(fs.readFileSync(baseline).equals(before));
     if(scenario==='eventual-success')assert.ok(calls>=2);
     const attachments=imageAttachments().map(a=>{
      const bytes=a.path&&fs.existsSync(a.path)?fs.readFileSync(a.path):a.body;
      return {name:a.name,contentType:a.contentType,exists:!!bytes,file:a.path?path.basename(a.path):undefined,sha256:bytes?crypto.createHash('sha256').update(bytes).digest('hex'):undefined,dimensions:bytes&&a.contentType==='image/png'?[bytes.readUInt32BE(16),bytes.readUInt32BE(20)]:undefined};
     });
     assert.ok(attachments.length>0);assert.ok(attachments.every(a=>a.exists&&a.contentType==='image/png'&&a.dimensions[0]===40&&a.dimensions[1]===30));
     if(scenario==='missing-target-failure')assert.ok(attachments.every(a=>a.name.includes('expected')));
     const settledProbes=probes.map(probe=>({...probe}));
     assert.ok(calls>=settledProbes.length);
     observation={name:scenario,outcome:error?'caught':'passed',error,durationMs:Math.round(performance.now()-start),baselineChanged:false,calls,unfinishedAtSettlement:calls-settledProbes.length,probes:settledProbes,attachments};
     cases.push(observation);
    }finally{await context.close()}
    observation.afterDisposal={calls,probes:probes.map(probe=>({...probe})),attachmentCount:imageAttachments().length};
   }
   assert.equal(cases.length,6);fs.appendFileSync(settings.events,JSON.stringify({name:engine,version:await browser.version(),cases})+'\n');
  }finally{await browser.close()}
 });
}
try{
 const events=join(root,'events.jsonl');await writeFile(events,'');
 await writeFile(join(root,'playwright.config.cjs'),"module.exports={testDir:'.',workers:1,retries:0,timeout:90000,updateSnapshots:'none',snapshotPathTemplate:'{snapshotDir}/{arg}{ext}',reporter:[['json']]};");
 const settings={testModule:require.resolve(moduleName+'/test'),browserModule:require.resolve(moduleName),chromium:process.env.FERRITE_CHROMIUM_PATH,firefox:process.env.FERRITE_FIREFOX_PATH||'/usr/bin/firefox',events};
 await writeFile(join(root,'snapshot-poll.spec.cjs'),'('+defineTests.toString()+')('+JSON.stringify(settings)+');');
 const run=spawnSync(process.execPath,[require.resolve(moduleName+'/cli'),'test','--config',join(root,'playwright.config.cjs')],{cwd:root,encoding:'utf8',timeout:180000,maxBuffer:16*1024*1024});
 if(run.error)throw run.error;assert.equal(run.status,0,run.stderr+'\n'+run.stdout);
 const report=JSON.parse(run.stdout);assert.equal(report.stats.expected,2);assert.equal(report.stats.unexpected,0);
 const engines=(await readFile(events,'utf8')).trim().split('\n').map(JSON.parse);assert.equal(engines.length,2);assert.equal(engines.reduce((total,e)=>total+e.cases.length,0),12);
 await writeFile(process.argv[2]||fileURLToPath(new URL('./snapshot-poll-reference.json',import.meta.url)),JSON.stringify({playwright:'1.63.0',engines},null,2)+'\n');
 console.log('Recorded 12 actual public screenshot/toPass attachment cases');
}finally{await rm(root,{recursive:true,force:true})}
