// Actual public runner assertions over the same original font/native animations.
import {createRequire} from 'node:module';
import {mkdtemp, writeFile, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import assert from 'node:assert/strict';
const require=createRequire(import.meta.url),moduleName=process.env.FERRITE_PLAYWRIGHT_MODULE||'playwright';
assert.equal(require(moduleName+'/package.json').version,'1.63.0');
const root=await mkdtemp(join(tmpdir(),'ferrite-font-animation-reference-'));

function defineTests(settings) {
 const {test,expect}=require(settings.testModule),pw=require(settings.browserModule),http=require('node:http'),fs=require('node:fs'),zlib=require('node:zlib'),assert=require('node:assert/strict');
 const font=fs.readFileSync(settings.font),css="@font-face{font-family:FerriteSnapshotProbe;src:url('/font.ttf') format('truetype');font-display:swap}html,body{margin:0}#probe{display:inline-block;font-family:FerriteSnapshotProbe,monospace;font-size:40px;line-height:40px;color:black}";
 const dimensions=bytes=>[bytes.readUInt32BE(16),bytes.readUInt32BE(20)];
 function pixel(bytes,x,y){
  const [width,height]=dimensions(bytes),channels=bytes[25]===6?4:bytes[25]===2?3:0;assert.equal(bytes[24],8);assert.ok(channels);assert.ok(x<width&&y<height);
  const chunks=[];for(let at=8;at<bytes.length;){const n=bytes.readUInt32BE(at);if(bytes.toString('ascii',at+4,at+8)==='IDAT')chunks.push(bytes.subarray(at+8,at+8+n));at+=n+12;}
  const raw=zlib.inflateSync(Buffer.concat(chunks)),stride=width*channels,paeth=(a,b,c)=>{const p=a+b-c,da=Math.abs(p-a),db=Math.abs(p-b),dc=Math.abs(p-c);return da<=db&&da<=dc?a:db<=dc?b:c;};
  let offset=0,previous=Buffer.alloc(stride);
  for(let rowIndex=0;rowIndex<=y;rowIndex++){
   const filter=raw[offset++],row=Buffer.alloc(stride);assert.ok(filter<=4);
   for(let i=0;i<stride;i++){const a=i>=channels?row[i-channels]:0,b=previous[i],c=i>=channels?previous[i-channels]:0;row[i]=(raw[offset++]+[0,a,b,Math.floor((a+b)/2),paeth(a,b,c)][filter])&255;}
   previous=row;
  }
  const at=x*channels;return [...previous.subarray(at,at+3),channels===4?previous[at+3]:255];
 }
 for(const engine of ['chromium','firefox'])test(engine,async({},info)=>{
  const launch=engine==='chromium'?{executablePath:settings.chromium}:{channel:'moz-firefox',executablePath:settings.firefox};
  const browser=await pw[engine].launch({...launch,timeout:30000}),cases=[];
  try {
   const fontCases=[['plain','page'],['plain','locator'],['shadow','page'],['shadow','locator'],['iframe','page'],['iframe','locator'],['plain','timeout']];
   for(const [kind,targetKind] of fontCases){
    let deliver,requested;
    const delivery=new Promise(resolve=>deliver=resolve),request=new Promise(resolve=>requested=resolve);
    const server=http.createServer(async(req,res)=>{
     res.setHeader('Cache-Control','no-store');res.setHeader('Connection','close');
     if(req.url==='/font.ttf'){requested();await delivery;res.setHeader('Content-Type','font/ttf');res.end(font);return;}
     res.setHeader('Content-Type','text/html');
     if(req.url==='/plain')res.end('<!doctype html><style>'+css+'</style><span id=probe>FFFF</span>');
     else if(req.url==='/shadow')res.end('<!doctype html><style>'+css+'</style><div id=host></div><script>document.getElementById("host").attachShadow({mode:"open"}).innerHTML="<style>#probe{display:inline-block;font-family:FerriteSnapshotProbe,monospace;font-size:40px;line-height:40px;color:black}</style><span id=probe>FFFF</span>"</script>');
     else if(req.url==='/iframe')res.end("<!doctype html><style>html,body{margin:0}iframe{border:0;width:250px;height:80px}</style><iframe src='/plain'></iframe>");
     else res.end('');
    });
    await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
    const context=await browser.newContext({viewport:{width:300,height:120}}),page=await context.newPage();
    try {
     await page.goto('http://127.0.0.1:'+server.address().port+'/'+kind,{waitUntil:'domcontentloaded'});await request;
     const probe=kind==='iframe'?page.frameLocator('iframe').locator('#probe'):page.locator('#probe');
     assert.equal(await probe.evaluate(el=>el.ownerDocument.fonts.status),'loading');
     const name=engine+'-font-'+kind+'-'+targetKind+'.png',baseline=info.snapshotPath(name,{kind:'screenshot'}),widthBefore=(await probe.boundingBox()).width;
     let settled=false,fontStatusAtSettlement;
     const start=performance.now(),capture=expect(targetKind==='page'?page:probe).toHaveScreenshot(name,{timeout:targetKind==='timeout'?350:3000,threshold:0,maxDiffPixels:0})
       .then(async()=>{settled=true;fontStatusAtSettlement=await probe.evaluate(el=>el.ownerDocument.fonts.status);return {outcome:'passed'}},error=>{settled=true;return {outcome:'caught',error:error.message.replace(/\u001b\[[0-9;]*m/g,'').split('\n')[0]}});
     await new Promise(resolve=>setTimeout(resolve,targetKind==='timeout'?450:500));
     const settledBeforeDelivery=settled,baselineBeforeDelivery=fs.existsSync(baseline);deliver();
     const result=await capture;
     await probe.evaluate(el=>el.ownerDocument.fonts.ready);
     const bytes=fs.existsSync(baseline)?fs.readFileSync(baseline):null;
     assert.equal(result.outcome,targetKind==='timeout'?'caught':'passed');
     if(targetKind==='timeout'){assert.equal(bytes,null);assert.equal(settledBeforeDelivery,true);}
     else assert.deepEqual(dimensions(bytes),targetKind==='page'?[300,120]:[160,40]);
     assert.equal(await probe.evaluate(el=>el.ownerDocument.fonts.check('40px FerriteSnapshotProbe')),true);
     assert.equal((await probe.boundingBox()).width,160);assert.notEqual(widthBefore,160);
     cases.push({name:'font-'+kind+'-'+targetKind,...result,durationMs:Math.round(performance.now()-start),widthBefore,widthAfter:160,settledBeforeDelivery,baselineBeforeDelivery,fontStatusAtSettlement,dimensions:bytes?dimensions(bytes):null});
    }finally{deliver();await context.close();server.closeAllConnections();await new Promise(resolve=>server.close(resolve));}
   }
   for(const kind of ['css-finite','css-infinite','waapi-finite','waapi-infinite','waapi-zero-rate']){
    const context=await browser.newContext({viewport:{width:120,height:80}}),page=await context.newPage();
    try {
     const infinite=kind.endsWith('infinite'),markup='<style>body{margin:0}@keyframes paint{from{background:red}to{background:blue}}#patch{width:80px;height:60px;background:red;'+(kind.startsWith('css')?'animation:paint 10s linear '+(infinite?'infinite':'1')+' forwards':'')+'}</style><div id=patch></div>';
     await page.setContent(markup);
     if(kind.startsWith('waapi'))await page.evaluate(({infinite,zero})=>{globalThis.animation=document.getElementById('patch').animate([{background:'red'},{background:'blue'}],{duration:10000,iterations:infinite?Infinity:1,fill:'forwards'});animation.finished.catch(()=>{});if(zero){animation.playbackRate=0;animation.currentTime=5000;}},{infinite,zero:kind.endsWith('zero-rate')});
     // Page clips isolate animation pixels from the locator action's native RAF stability wait.
     const name=engine+'-'+kind+'.png';await expect(page).toHaveScreenshot(name,{clip:{x:0,y:0,width:80,height:60},timeout:3000,threshold:0,maxDiffPixels:0});
     const bytes=fs.readFileSync(info.snapshotPath(name,{kind:'screenshot'})),color=pixel(bytes,5,5),state=await page.evaluate(()=>document.getAnimations()[0]?.playState),rate=await page.evaluate(()=>document.getAnimations()[0]?.playbackRate);
     assert.deepEqual(dimensions(bytes),[80,60]);
     if(!kind.endsWith('zero-rate')){assert.deepEqual(color,infinite?[255,0,0,255]:[0,0,255,255]);assert.equal(state,infinite?'running':'finished');}
     else assert.equal(rate,0);
     cases.push({name:kind,captureTarget:'page-clip',outcome:'passed',dimensions:dimensions(bytes),pixel:color,state,rate});
     if(kind.startsWith('css')){
      await page.setContent(markup);const locatorName=engine+'-'+kind+'-locator.png';let locatorError;
      try{await expect(page.locator('#patch')).toHaveScreenshot(locatorName,{timeout:2000,threshold:0,maxDiffPixels:0})}catch(caught){assert.match(caught.message,/waiting for element to be stable/);assert.match(caught.message,/Timeout/);locatorError=caught.message.replace(/\u001b\[[0-9;]*m/g,'').split('\n')[0];}
      const locatorPath=info.snapshotPath(locatorName,{kind:'screenshot'});
      if(locatorError){assert.equal(fs.existsSync(locatorPath),false);cases.push({name:kind+'-locator',captureTarget:'locator',outcome:'caught',reason:'native-locator-stability-timeout',error:locatorError,baselineExists:false});}
      else{const locatorBytes=fs.readFileSync(locatorPath);assert.deepEqual(dimensions(locatorBytes),[80,60]);assert.deepEqual(pixel(locatorBytes,5,5),infinite?[255,0,0,255]:[0,0,255,255]);cases.push({name:kind+'-locator',captureTarget:'locator',outcome:'passed',dimensions:dimensions(locatorBytes),pixel:pixel(locatorBytes,5,5)});}
      await page.setContent(markup);const live=engine+'-'+kind+'-live.png';let error;
      try{await expect(page).toHaveScreenshot(live,{clip:{x:0,y:0,width:80,height:60},timeout:650,animations:'allow',threshold:0,maxDiffPixels:0})}catch(caught){error=caught.message.replace(/\u001b\[[0-9;]*m/g,'').split('\n')[0];}
      assert.ok(error);assert.equal(fs.existsSync(info.snapshotPath(live,{kind:'screenshot'})),false);
      cases.push({name:kind+'-live',captureTarget:'page-clip',outcome:'caught',error,baselineExists:false});
     }
    }finally{await context.close();}
   }
   assert.equal(cases.length,16);fs.appendFileSync(settings.events,JSON.stringify({name:engine,version:await browser.version(),cases})+'\n');
  }finally{await browser.close();}
 });
}
try {
 const events=join(root,'events.jsonl');await writeFile(events,'');
 await writeFile(join(root,'playwright.config.cjs'),`module.exports={testDir:'.',workers:1,retries:0,timeout:90000,updateSnapshots:'all',snapshotPathTemplate:'{snapshotDir}/{arg}{ext}',reporter:[['json']]};`);
 const settings={testModule:require.resolve(moduleName+'/test'),browserModule:require.resolve(moduleName),font:fileURLToPath(new URL('../../crates/ferrite-e2e/tests/fixtures/snapshot-probe.ttf',import.meta.url)),chromium:process.env.FERRITE_CHROMIUM_PATH,firefox:process.env.FERRITE_FIREFOX_PATH||'/usr/bin/firefox',events};
 await writeFile(join(root,'font-animation.spec.cjs'),'('+defineTests.toString()+')('+JSON.stringify(settings)+');');
 const run=spawnSync(process.execPath,[require.resolve(moduleName+'/cli'),'test','--config',join(root,'playwright.config.cjs')],{cwd:root,encoding:'utf8',timeout:180000,maxBuffer:16*1024*1024});
 if(run.error)throw run.error;assert.equal(run.status,0,run.stderr+'\n'+run.stdout);
 const report=JSON.parse(run.stdout);assert.equal(report.stats.expected,2);assert.equal(report.stats.unexpected,0);
 const engines=(await readFile(events,'utf8')).trim().split('\n').map(JSON.parse);assert.equal(engines.length,2);assert.equal(engines.reduce((total,e)=>total+e.cases.length,0),32);
 await writeFile(process.argv[2]||fileURLToPath(new URL('./font-animation-reference.json',import.meta.url)),JSON.stringify({playwright:'1.63.0',fontSha256:'eb22d9ac770bd9abf4e4caf1c5285ac033d76df7171ab6fbb170d1d13113748c',engines},null,2)+'\n');
 console.log('Recorded 32 actual public font/animation assertion cases');
}finally{await rm(root,{recursive:true,force:true});}
