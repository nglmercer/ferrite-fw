import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
const require=createRequire(import.meta.url);
const modulePath=process.env.FERRITE_PLAYWRIGHT_MODULE||'playwright';
const {chromium}=require(modulePath);
const version=require(modulePath+'/package.json').version;
if(version!=='1.63.0')throw Error('Reference requires Playwright 1.63.0');
const server=createServer((req,res)=>{
 const url=new URL(req.url,'http://fixture');
 if(url.pathname==='/ping'){res.end('ok');return;}
 res.setHeader('content-type','text/html');
 if(!url.pathname.startsWith('/popup/')){res.end('<!doctype html><title>opener</title>');return;}
 const name=url.pathname.slice('/popup/'.length);
 const marker=JSON.stringify(name);
 const script=name.startsWith('close')
 ?`console.log('startup:'+${marker});const request=new XMLHttpRequest();request.open('GET','/ping?case='+${marker},false);request.send();window.close();`
 :`console.log('startup:'+${marker});fetch('/ping?case='+${marker}).then(r=>r.text()).then(()=>window.ready=true);queueMicrotask(()=>{throw new Error('startup-error')});`;
 res.end(`<!doctype html><title>popup</title><script>${script}</script>`);
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const base=`http://127.0.0.1:${server.address().port}`;
let browser;
try {
 browser=await chromium.launch({executablePath:process.env.FERRITE_CHROMIUM_PATH,headless:true});
 const context=await browser.newContext();const opener=await context.newPage();await opener.goto(base);
 const records=new Map();
 function record(page){if(!records.has(page))records.set(page,{console:[],contextConsole:[],errors:[],requests:[],closed:0});return records.get(page);}
 context.on('page',page=>{
  const entry=record(page);
  page.on('console',message=>entry.console.push(message.text()));
  page.on('pageerror',error=>entry.errors.push(error.message));
  page.on('close',()=>entry.closed++);
 });
 context.on('console',message=>record(message.page()).contextConsole.push(message.text()));
 let unavailableNavigationFrames=0;
 context.on('request',request=>{
  let page;
  try {page=request.frame().page();}
  catch(error){if(!request.isNavigationRequest())throw error;unavailableNavigationFrames++;return;}
  record(page).requests.push(request.url());
 });
 const opened=opener.waitForEvent('popup');await opener.evaluate(()=>{window.open('/popup/live');return true;});
 const live=await opened;await live.waitForFunction(()=>window.ready===true);
 await live.close();
 const liveRecord=record(live);
 const cases=[{name:'live-startup',result:{console:liveRecord.console.filter(text=>text==='startup:live').length,contextConsole:liveRecord.contextConsole.filter(text=>text==='startup:live').length,errors:liveRecord.errors.filter(text=>text.includes('startup-error')).length,ping:liveRecord.requests.filter(url=>url.endsWith('/ping?case=live')).length,closed:liveRecord.closed,opener:await live.opener()===opener}}];
 const closed=[];
 const ready=new Promise(resolve=>context.on('page',page=>page.on('close',()=>{if(page===live)return;closed.push(page);if(closed.length===4)resolve();})));
 await opener.evaluate(()=>{for(let i=0;i<4;i++)window.open('/popup/close-'+i);});
 await Promise.race([ready,new Promise((_,reject)=>setTimeout(()=>reject(Error('immediate popup close timeout')),8000))]);
 const entries=[];
 for(let index=0;index<4;index++){
  const name=`close-${index}`;
  const page=closed.find(page=>record(page).console.includes(`startup:${name}`));
  if(!page)throw Error(`missing startup log for ${name}`);
  const entry=record(page);
  entries.push({case:name,console:entry.console.filter(text=>text===`startup:${name}`).length,contextConsole:entry.contextConsole.filter(text=>text===`startup:${name}`).length,ping:entry.requests.filter(url=>url.endsWith(`/ping?case=${name}`)).length,closed:entry.closed,opener:await page.opener()===opener});
 }
 cases.push({name:'concurrent-immediate-close',result:entries});
 const output={playwright:version,engine:'chromium',browser:browser.version(),unavailableNavigationFrames,cases};
 const target=process.argv[2]||fileURLToPath(new URL('./popup-reference.json',import.meta.url));
 await writeFile(target,JSON.stringify(output,null,2)+'\n');
 console.log(`Recorded ${cases.length} popup cases on Playwright ${version}, Chromium ${output.browser}`);
} finally {if(browser)await browser.close();await new Promise(resolve=>server.close(resolve));}
