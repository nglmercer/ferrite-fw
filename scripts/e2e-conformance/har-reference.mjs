// Pinned HAR duplicate matching, miss policy and native recording metadata.
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {mkdtemp,readFile,writeFile,rm} from 'node:fs/promises';
import {join} from 'node:path';import {tmpdir} from 'node:os';
import assert from 'node:assert/strict';
const require=createRequire(import.meta.url),moduleName=process.env.FERRITE_PLAYWRIGHT_MODULE||'playwright';
assert.equal(require(moduleName+'/package.json').version,'1.63.0');const {chromium}=require(moduleName);
let hits=0;
const server=createServer((request,response)=>{
  if(request.url==='/binary'){response.setHeader('Content-Type','application/octet-stream');response.end(Buffer.from([0,255,128,65]));}
  else if(request.url==='/miss'){hits++;response.end('network');}
  else {response.setHeader('Content-Type','text/html');response.end('<title>HAR</title>');}
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));const base=`http://127.0.0.1:${server.address().port}`;
const root=await mkdtemp(join(tmpdir(),'ferrite-har-reference-'));
const entry=(path,text,headers=[],postData)=>({request:{method:postData===undefined?'GET':'POST',url:base+path,headers,cookies:[],queryString:[],postData:postData===undefined?undefined:{text:postData}},response:{status:200,statusText:'OK',headers:[{name:'Content-Type',value:'text/plain'}],content:{text,mimeType:'text/plain'},redirectURL:''}});
const entries=[entry('/choice','first',[{name:'X-Choice',value:'one'}]),entry('/choice','second',[{name:'X-Choice',value:'two'}]),entry('/choice','tied',[{name:'X-Choice',value:'two'}]),entry('/post','posted',[],'payload')];
const binary=entry('/replay-binary','');binary.response.content={text:'AP+AQQ==',encoding:'base64',mimeType:'application/octet-stream'};entries.push(binary);
const path=join(root,'seed.har');await writeFile(path,JSON.stringify({log:{version:'1.2',creator:{name:'fixture',version:'1'},entries}}));
let browser;const replay=[],recording=[];
try {
  browser=await chromium.launch({executablePath:process.env.FERRITE_CHROMIUM_PATH});
  const page=await browser.newPage();await page.goto(base);
  await page.routeFromHAR(path,{url:`${base}/{choice,post,replay-binary,miss}`,notFound:'abort'});
  assert.equal(await page.evaluate(()=>fetch('/choice',{headers:{'X-Choice':'two'}}).then(r=>r.text())),'second');
  assert.equal(await page.evaluate(()=>fetch('/post',{method:'POST',body:'payload'}).then(r=>r.text())),'posted');
  assert.deepEqual(await page.evaluate(()=>fetch('/replay-binary').then(r=>r.arrayBuffer()).then(b=>Array.from(new Uint8Array(b)))),[0,255,128,65]);
  const before=hits;assert.equal(await page.evaluate(()=>fetch('/miss').then(()=>false).catch(()=>true)),true);assert.equal(hits,before);
  replay.push({notFound:'abort',duplicateWinner:'second',tie:'first matching file candidate',post:'payload',binary:[0,255,128,65],missReachedNetwork:false});
  await page.unrouteAll();await page.routeFromHAR(path,{url:base+'/miss',notFound:'fallback'});
  assert.equal(await page.evaluate(()=>fetch('/miss').then(r=>r.text())),'network');replay.push({notFound:'fallback',missReachedNetwork:true});
  await page.close();
  for(const mode of ['full','minimal'])for(const content of ['embed','omit']){
    const path=join(root,`${mode}-${content}.har`),context=await browser.newContext({recordHar:{path,mode,content}});
    const page=await context.newPage();await page.goto(base);await page.evaluate(()=>fetch('/binary').then(r=>r.arrayBuffer()));await context.close();
    const document=JSON.parse(await readFile(path,'utf8')),entry=document.log.entries.find(entry=>entry.request.url===base+'/binary');assert.ok(entry);
    if(content==='embed'){assert.equal(entry.response.content.encoding,'base64');assert.equal(entry.response.content.text,'AP+AQQ==');}else{assert.equal(entry.response.content.text,undefined);}
    recording.push({mode,content,entryFields:Object.keys(entry).sort(),requestFields:Object.keys(entry.request).sort(),responseFields:Object.keys(entry.response).sort(),timings:entry.timings,encodedBinary:entry.response.content.text??null});
  }
  await writeFile(new URL('har-reference.json',import.meta.url),JSON.stringify({playwright:'1.63.0',engine:'chromium',replay,recording,note:'Ferrite preserves legacy fallback default; abort is explicit. Ferrite export is an explicit Page snapshot with bounded native/captured data and approximate total timing, not automatic context recordHar. ZIP/update/attached content remain deferred.'},null,2)+'\n');
}finally{if(browser)await browser.close();await new Promise(resolve=>server.close(resolve));await rm(root,{recursive:true,force:true});}
console.log('Passed pinned HAR replay policies, duplicate/body/header selection and four recording profiles.');
