import {createRequire} from 'node:module';
import {writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
const require=createRequire(import.meta.url);
const modulePath=process.env.FERRITE_PLAYWRIGHT_MODULE||'playwright';
const {chromium}=require(modulePath);
const version=require(modulePath+'/package.json').version;
if(version!=='1.63.0')throw Error('Reference requires Playwright 1.63.0');
const browser=await chromium.launch({executablePath:process.env.FERRITE_CHROMIUM_PATH,headless:true});
try {
 const page=await browser.newPage();
 const consolePromise=page.waitForEvent('console');const errorPromise=page.waitForEvent('pageerror');
 await page.evaluate(()=>{
  const cyclic={answer:42};cyclic.self=cyclic;
  console.log('metadata',undefined,null,NaN,Infinity,-0,12n,Symbol('mark'),function named(){},cyclic,{array:[1,'x'],obj:{nested:true}},new Error('console error'));
  function outer(){function inner(){const error=new TypeError('structured error');error.name='RenamedError';throw error;}inner();}
  setTimeout(outer,0);
 });
 const consoleMessage=await consolePromise;const error=await errorPromise;
 const primitives=[];
 for(const handle of consoleMessage.args().slice(0,9)){
  primitives.push(await handle.evaluate(value=>{
   if(value===null)return {kind:'null',value:null};
   const kind=typeof value;
   if(kind==='number'&&!Number.isFinite(value))return {kind,special:String(value)};
   if(kind==='number'&&Object.is(value,-0))return {kind,special:'-0'};
   if(kind==='bigint')return {kind,special:String(value)};
   if(['undefined','symbol','function'].includes(kind))return {kind};
   return {kind,value};
  }));
 }
 const cases=[{name:'primitive-arguments',result:primitives},{name:'mutable-error-name',result:{name:error.name,message:error.message,inner:error.stack.includes('inner'),outer:error.stack.includes('outer')}}];
 const output={playwright:version,engine:'chromium',browser:browser.version(),cases};
 await writeFile(process.argv[2]||fileURLToPath(new URL('./console-reference.json',import.meta.url)),JSON.stringify(output,null,2)+'\n');
 console.log(`Recorded ${cases.length} console/error cases on Playwright ${version}, Chromium ${output.browser}`);
} finally {await browser.close();}
