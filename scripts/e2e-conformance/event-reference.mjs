import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
const require=createRequire(import.meta.url);
const modulePath=process.env.FERRITE_PLAYWRIGHT_MODULE||'playwright';
const {chromium}=require(modulePath);
const version=require(modulePath+'/package.json').version;
if(version!=='1.63.0')throw Error('Reference requires Playwright 1.63.0');
const server=createServer((req,res)=>{res.setHeader('content-type','text/html');res.end('<!doctype html><title>events</title><body>fixture</body>');});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const base=`http://127.0.0.1:${server.address().port}`;
let browser;
try {
 browser=await chromium.launch({executablePath:process.env.FERRITE_CHROMIUM_PATH,headless:true});
 const page=await browser.newPage();
 const main=page.mainFrame();
 const labels=new Map([[main,'main']]);
 const parents=new Map([[main,null]]);
 const events=[];
 let next=0;
 function label(frame){if(!labels.has(frame))labels.set(frame,`f${++next}`);return labels.get(frame);}
 function record(kind,frame){
  if(kind==='attached')parents.set(frame,label(frame.parentFrame()));
  const url=frame.url();
  events.push({kind,frame:label(frame),parent:parents.get(frame)??null,path:url.startsWith(base)?url.slice(base.length):null});
 }
 page.on('frameattached',frame=>record('attached',frame));
 page.on('framenavigated',frame=>record('navigated',frame));
 page.on('framedetached',frame=>record('detached',frame));
 page.on('domcontentloaded',()=>record('dom',main));
 page.on('load',()=>record('load',main));
 await page.goto(base+'/main');
 await page.evaluate(()=>new Promise(resolve=>{const frame=document.createElement('iframe');frame.id='outer';frame.onload=()=>resolve(true);frame.src='/frame?phase=1';document.body.append(frame);}));
 await page.evaluate(()=>new Promise(resolve=>{const doc=document.querySelector('#outer').contentDocument;const frame=doc.createElement('iframe');frame.onload=()=>resolve(true);frame.src='/nested';doc.body.append(frame);}));
 await page.evaluate(()=>new Promise(resolve=>{const frame=document.querySelector('#outer');frame.onload=()=>resolve(true);frame.src='/frame?phase=2';}));
 await page.evaluate(()=>{location.hash='fragment';});
 await page.waitForURL(base+'/main#fragment');
 await page.evaluate(()=>{history.pushState({},'', '/main?history=1');});
 await page.waitForURL(base+'/main?history=1');
 await page.evaluate(()=>{history.pushState({},'', '/main?history=1');});
 await page.evaluate(()=>document.querySelector('#outer').remove());
 await page.evaluate(()=>new Promise(resolve=>{const frame=document.createElement('iframe');frame.id='outer';frame.onload=()=>resolve(true);frame.src='/frame?phase=3';document.body.append(frame);}));
 await page.evaluate(()=>new Promise(resolve=>{const doc=document.querySelector('#outer').contentDocument;const frame=doc.createElement('iframe');frame.onload=()=>resolve(true);frame.src='/nested?phase=2';doc.body.append(frame);}));
 await page.evaluate(()=>document.querySelector('#outer').remove());
 const frames=[];
 for(const [frame,id]of labels){
  const rows=events.filter(event=>event.frame===id);
  frames.push({frame:id,parent:parents.get(frame)??null,attached:rows.filter(event=>event.kind==='attached').length,detached:rows.filter(event=>event.kind==='detached').length,navigations:rows.filter(event=>event.kind==='navigated'&&event.path!==null).map(event=>event.path),ready:rows.filter(event=>event.kind==='dom'||event.kind==='load').map(event=>event.kind)});
 }
 const output={playwright:version,engine:'chromium',browser:browser.version(),cases:[{name:'frame-lifecycle',result:frames}]};
 const start=events.length;
 for(let index=1;index<=2;index++)await page.setContent(`<!doctype html><title>content ${index}</title><body>replacement</body>`);
 output.cases.push({name:'repeated-readiness',result:{sameMain:page.mainFrame()===main,ready:events.slice(start).filter(event=>event.kind==='dom'||event.kind==='load').map(event=>event.kind)}});
 const pageCloses=[];const contextCloses=[];
 page.on('dialogclosed',dialog=>pageCloses.push(dialog));
 page.context().on('dialogclosed',dialog=>contextCloses.push(dialog));
 page.once('dialog',dialog=>dialog.accept('fixture'));
 const prompt=await page.evaluate(()=>prompt('accept prompt','default'));
 output.cases.push({name:'prompt-accepted',result:{accepted:prompt!==null,userText:prompt,pageClosed:pageCloses.length,contextClosed:contextCloses.length,type:pageCloses[0].type(),pageOwned:pageCloses[0].page()===page}});
 page.once('dialog',dialog=>dialog.dismiss());
 const confirm=await page.evaluate(()=>confirm('dismiss confirm'));
 output.cases.push({name:'confirm-dismissed',result:{accepted:confirm,pageClosed:pageCloses.length-1,contextClosed:contextCloses.length-1,type:pageCloses[1].type(),pageOwned:pageCloses[1].page()===page}});
 const target=process.argv[2]||fileURLToPath(new URL('./event-reference.json',import.meta.url));
 await writeFile(target,JSON.stringify(output,null,2)+'\n');
 console.log(`Recorded ${output.cases.length} lifecycle cases on Playwright ${version}, Chromium ${output.browser}`);
} finally {if(browser)await browser.close();await new Promise(resolve=>server.close(resolve));}
