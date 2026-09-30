import {createRequire} from 'node:module';
import {mkdtemp, writeFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
const require=createRequire(import.meta.url);
const modulePath=process.env.FERRITE_PLAYWRIGHT_MODULE||'playwright';
const version=require(modulePath+'/package.json').version;
if(version!=='1.63.0')throw Error('Reference requires Playwright 1.63.0');
const testModule=require.resolve(modulePath+'/test');
const cli=require.resolve(modulePath+'/cli');
const root=await mkdtemp(join(tmpdir(),'ferrite-configuration-reference-'));
try {
 await writeFile(join(root,'playwright.config.cjs'),`module.exports={testDir:'.',workers:2,timeout:10000,retries:2,repeatEach:3,reporter:'json',use:{viewport:{width:360,height:240},headless:true,launchOptions:{executablePath:${JSON.stringify(process.env.FERRITE_CHROMIUM_PATH)}}},projects:[{name:'alpha',grep:/focus/,grepInvert:/blocked/,repeatEach:2,retries:1,timeout:8000,use:{viewport:{width:480,height:320}}},{name:'beta',grep:/focus/,grepInvert:/blocked/,repeatEach:1,retries:0,timeout:0,use:{viewport:{width:600,height:400}}}]};`);
 await writeFile(join(root,'configuration.spec.cjs'),`const {test}=require(${JSON.stringify(testModule)});
 test('probe @focus',async ({page},info)=>{
 const initial={project:info.project.name,repeat:info.repeatEachIndex,retries:info.project.retries,repeatEach:info.project.repeatEach,timeout:info.timeout,viewport:await page.evaluate(()=>[innerWidth,innerHeight])};
 test.setTimeout(9000);
 await info.attach('settings',{body:Buffer.from(JSON.stringify({...initial,currentTimeout:info.timeout})),contentType:'application/json'});
 });
 test('excluded @focus @blocked',async()=>{throw Error('project exclusion failed');});
 test('not selected',async()=>{throw Error('project inclusion failed');});`);
 const run=(extra=[])=>{
  const result=spawnSync(process.execPath,[cli,'test','--config',join(root,'playwright.config.cjs'),...extra],{cwd:root,encoding:'utf8',maxBuffer:16*1024*1024});
  if(result.error)throw result.error;
  if(result.status!==0)throw Error(result.stderr+'\n'+result.stdout);
  const report=JSON.parse(result.stdout),cases=[];
  const visit=suite=>{for(const spec of suite.specs||[])for(const test of spec.tests||[])for(const attempt of test.results||[]){const attachment=attempt.attachments.find(a=>a.name==='settings');if(!attachment?.body)throw Error('Missing configuration observation');cases.push(JSON.parse(Buffer.from(attachment.body,'base64').toString('utf8')));}for(const nested of suite.suites||[])visit(nested);};
  for(const suite of report.suites)visit(suite);
  return cases.sort((a,b)=>a.project.localeCompare(b.project)||a.repeat-b.repeat);
 };
 const cases=run(),selected=run(['--project=beta']);
 if(cases.length!==3||selected.length!==1)throw Error('Incomplete configuration reference');
 const target=process.argv[2]||fileURLToPath(new URL('./configuration-reference.json',import.meta.url));
 await writeFile(target,JSON.stringify({playwright:version,engine:'chromium',cases,selected},null,2)+'\n');
 console.log(`Recorded ${cases.length} project/repetition cases and ${selected.length} selected-project case from Playwright ${version}`);
}finally {await rm(root,{recursive:true,force:true});}
