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
const root=await mkdtemp(join(tmpdir(),'ferrite-soft-reference-'));
try {
 await writeFile(join(root,'playwright.config.cjs'),`module.exports={testDir:'.',workers:2,retries:1,reporter:'json',use:{headless:true,launchOptions:{executablePath:${JSON.stringify(process.env.FERRITE_CHROMIUM_PATH)}}}};`);
 await writeFile(join(root,'soft.spec.cjs'),`const {test,expect}=require(${JSON.stringify(testModule)});
 test.describe.configure({mode:'parallel'});
 test.afterEach(async ({},info)=>{ if(info.title==='cleanup mismatch')expect.soft(1,'cleanup soft').toBe(2); });
 test('retry mismatches',async ({page},info)=>{ await page.setContent('<h1>Actual</h1>');
 if(info.retry===0){await expect.soft(page.locator('h1'),'first message').toHaveText('wrong',{timeout:60});expect.soft(1,'second message').toBe(2);}
 await info.attach('continued',{body:Buffer.from('yes'),contentType:'text/plain'}); });
 test('expected mismatch',async()=>{test.fail();expect.soft(1,'expected soft').toBe(2);});
 test('cleanup mismatch',async()=>{});
 test('parallel clean',async()=>{expect(1).toBe(1);});
 test('skip after mismatch',async()=>{expect.soft(1,'before skip').toBe(2);test.skip();});`);
 const result=spawnSync(process.execPath,[cli,'test','--config',join(root,'playwright.config.cjs')],{cwd:root,encoding:'utf8',maxBuffer:16*1024*1024});
 if(result.error)throw result.error;
 if(![0,1].includes(result.status))throw Error(result.stderr);
 const report=JSON.parse(result.stdout); const cases=[];
 const visit=suite=>{for(const spec of suite.specs||[])for(const test of spec.tests||[])cases.push({name:spec.title,result:{expected:test.expectedStatus,attempts:test.results.map(attempt=>({status:attempt.status,errors:attempt.errors.length,continued:(attempt.attachments||[]).some(a=>a.name==='continued')}))}});for(const nested of suite.suites||[])visit(nested);};
 for(const suite of report.suites)visit(suite);
 if(cases.length!==5)throw Error('Incomplete runner reference');
 cases.sort((a,b)=>a.name.localeCompare(b.name));
 const target=process.argv[2]||fileURLToPath(new URL('./soft-assertion-reference.json',import.meta.url));
 await writeFile(target,JSON.stringify({playwright:version,engine:'chromium',cases},null,2)+'\n');
 console.log(`Recorded ${cases.length} actual Playwright ${version} runner cases`);
}finally {await rm(root,{recursive:true,force:true});}
