import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {mkdtemp,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
const require=createRequire(import.meta.url);
const modulePath=process.env.FERRITE_PLAYWRIGHT_MODULE||'playwright';
const {chromium}=require(modulePath);
const version=require(modulePath+'/package.json').version;
if(version!=='1.63.0')throw Error('Reference requires Playwright 1.63.0');
let resets=0;
const server=createServer(async(req,res)=>{
 const body=[];for await(const bytes of req)body.push(bytes);
 if(req.url==='/redirect'){res.writeHead(302,{location:'/echo','set-cookie':'redirected=1; Path=/'});res.end('redirect');return;}
 if(req.url==='/reset'&&resets++===0){req.socket.destroy();return;}
 if(req.url==='/source'){const body=Buffer.from([0,255,128,13,10]);res.writeHead(200,{'content-type':'application/octet-stream','content-length':body.length,'x-source':'original','set-cookie':['one=1; Path=/','two=2; Path=/']});res.end(body);return;}
 if(req.url==='/echo'){res.writeHead(200,{'content-type':'application/json','set-cookie':'fetched=1; Path=/'});res.end(JSON.stringify({method:req.method,bytes:[...Buffer.concat(body)],type:req.headers['content-type']||null,extra:req.headers['x-extra']||null,cookie:req.headers.cookie||null}));return;}
 res.end('fixture');
});await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const base=`http://127.0.0.1:${server.address().port}`;
const directory=await mkdtemp(join(tmpdir(),'ferrite-route-reference-'));
const file=join(directory,'fixture.txt');await writeFile(file,'file bytes');
let browser;
try{
 browser=await chromium.launch({executablePath:process.env.FERRITE_CHROMIUM_PATH,headless:true});
 const context=await browser.newContext({baseURL:base,extraHTTPHeaders:{'x-extra':'context'}});
 await context.addCookies([{name:'before',value:'1',url:base}]);
 const page=await context.newPage();await page.goto(base);
 const output={playwright:version,engine:'chromium',browser:browser.version(),cases:[]};
 async function run(name,handler,body){
  let details;
  await page.route('**/target',async route=>{details=await handler(route);});
  const result=await page.evaluate(async body=>{
   const response=await fetch('/target',body===null?{}:{method:'POST',body:new Uint8Array(body)});
   return {status:response.status,type:response.headers.get('content-type'),length:response.headers.get('content-length'),source:response.headers.get('x-source'),only:response.headers.get('x-only'),bytes:[...new Uint8Array(await response.arrayBuffer())]};
  },body===undefined?null:body);
  output.cases.push({name,result,details:details??null});await page.unrouteAll();
 }
 await run('fetch-original-binary',async route=>{const response=await route.fetch({url:base+'/echo'});const echo=await response.json();await route.fulfill({json:echo});return {method:echo.method,bytes:echo.bytes,type:echo.type,extra:echo.extra,cookieNames:echo.cookie.split(';').map(s=>s.trim().split('=')[0]).sort()};},[0,255,128,13,10]);
 await run('fetch-overrides',async route=>{const response=await route.fetch({url:base+'/echo',method:'put',postData:Buffer.from([0,255,10]),headers:{'content-type':'application/octet-stream','content-length':'999'}});const echo=await response.json();await route.fulfill({json:echo});return {method:echo.method,bytes:echo.bytes,type:echo.type,extra:echo.extra};});
 await run('fetch-json',async route=>{const response=await route.fetch({url:'/echo',method:'POST',postData:{changed:true},headers:{}});const echo=await response.json();await route.fulfill({json:echo});return {method:echo.method,bytes:echo.bytes,type:echo.type,extra:echo.extra};});
 await run('fetch-no-redirect',async route=>{const response=await route.fetch({url:base+'/redirect',maxRedirects:0});const body=await response.text();await route.fulfill({body});return {status:response.status(),body};});
 await run('fetch-reset-retry',async route=>{const response=await route.fetch({url:base+'/reset',maxRetries:1});await route.fulfill({response});return {status:response.status(),requests:resets};});
 for(const [name,options]of[
  ['response-inherit',{}],
  ['response-status',{status:201}],
  ['response-body',{body:'new body longer'}],
  ['response-headers',{headers:{'x-only':'replacement'}}],
  ['json-header-precedence',{json:{changed:true},headers:{'content-type':'text/plain'}}],
  ['json-false',{json:false,headers:{'content-type':'text/plain'}}],
  ['json-null',{json:null,headers:{'content-type':'text/plain'}}],
  ['json-explicit-type',{json:{changed:true},headers:{'content-type':'text/plain'},contentType:'application/x-fixture'}],
  ['file-header-precedence',{path:file,headers:{'content-type':'application/octet-stream'}}],
  ['file-over-body',{path:file,body:'ignored'}],
  ['file-over-json',{path:file,json:{ignored:true}}],
 ])await run(name,async route=>{const response=await context.request.get(base+'/source');await route.fulfill({response,...options});});
 for(const explicit of [false,true]){
  await page.route('**/cors-target',async route=>{
   const headers={'access-control-expose-headers':'Access-Control-Allow-Origin, Access-Control-Allow-Credentials, Vary'};
   if(explicit)headers['access-control-allow-origin']='*';
   await route.fulfill({body:'cors bytes',headers});
  });
  const result=await page.evaluate(async({url,explicit})=>{
   const response=await fetch(url,{credentials:explicit?'omit':'include'});
   return {status:response.status,bytes:[...new Uint8Array(await response.arrayBuffer())],originAccepted:response.headers.get('access-control-allow-origin')===(explicit?'*':location.origin),credentials:response.headers.get('access-control-allow-credentials'),vary:response.headers.get('vary')};
  },{url:base.replace('127.0.0.1','localhost')+'/cors-target',explicit});
  output.cases.push({name:explicit?'cors-explicit-headers':'cors-contextual-fulfill',result,details:null});await page.unrouteAll();
 }
 const target=process.argv[2]||fileURLToPath(new URL('./route-options-reference.json',import.meta.url));
 await writeFile(target,JSON.stringify(output,null,2)+'\n');console.log(`Recorded ${output.cases.length} route option cases on Playwright ${version}, Chromium ${output.browser}`);
 await context.close();
}finally{if(browser)await browser.close();await new Promise(resolve=>server.close(resolve));await rm(directory,{recursive:true,force:true});}
