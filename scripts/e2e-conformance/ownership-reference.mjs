import {createRequire} from 'node:module';
import {mkdtemp, rm, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
const require=createRequire(import.meta.url);
const modulePath=process.env.FERRITE_PLAYWRIGHT_MODULE||'playwright';
const {chromium}=require(modulePath);
const version=require(modulePath+'/package.json').version;
if(version!=='1.63.0')throw Error('Reference requires Playwright 1.63.0');
const launch={executablePath:process.env.FERRITE_CHROMIUM_PATH,headless:true};
let browser,persistent;
const profile=await mkdtemp(join(tmpdir(),'ferrite-owner-reference-'));
try {
 browser=await chromium.launch(launch);
 const context=await browser.newContext();
 const page=await browser.newPage();const convenience=page.context();
 const explicitOwner=context.browser()===browser;
 const convenienceOwner=convenience.browser()===browser;
 let convenienceClosed=false;convenience.on('close',()=>{convenienceClosed=true;});
 await page.close();
 const connected=browser.isConnected();
 await context.close();
 const cases=[{name:'context-owners',result:{explicitOwner,convenienceOwner,connected,convenienceClosed,closedContextOwner:context.browser()===browser}}];
 const browserVersion=browser.version();await browser.close();browser=undefined;
 persistent=await chromium.launchPersistentContext(profile,launch);
 const owner=persistent.browser();const persistentOwner=owner!==null;
 await persistent.close();persistent=undefined;
 cases.push({name:'persistent-owner',result:{persistentOwner,disconnected:owner===null?null:!owner.isConnected()}});
 const output={playwright:version,engine:'chromium',browser:browserVersion,cases};
 const target=process.argv[2]||fileURLToPath(new URL('./ownership-reference.json',import.meta.url));
 await writeFile(target,JSON.stringify(output,null,2)+'\n');
 console.log(`Recorded ${cases.length} ownership cases on Playwright ${version}, Chromium ${browserVersion}`);
} finally {
 if(persistent)await persistent.close();if(browser)await browser.close();
 await rm(profile,{recursive:true,force:true});
}
