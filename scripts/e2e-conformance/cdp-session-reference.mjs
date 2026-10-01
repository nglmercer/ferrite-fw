// Pinned public target-session lifecycle observations.
import {createRequire} from 'node:module';import {writeFile} from 'node:fs/promises';import assert from 'node:assert/strict';
const require=createRequire(import.meta.url),moduleName=process.env.FERRITE_PLAYWRIGHT_MODULE||'playwright';assert.equal(require(moduleName+'/package.json').version,'1.63.0');const {chromium}=require(moduleName);
const browser=await chromium.launch({executablePath:process.env.FERRITE_CHROMIUM_PATH});const observations={playwright:'1.63.0',engine:'chromium'};
try{
 const page=await browser.newPage(),other=await browser.newPage();const first=await page.context().newCDPSession(page),second=await page.context().newCDPSession(page);let closes=0;first.on('close',()=>closes++);const events=[];first.on('event',event=>events.push(event.method));await first.send('Runtime.enable');assert(events.includes('Runtime.executionContextCreated'));
 const pending=first.send('Runtime.evaluate',{expression:'new Promise(()=>{})',awaitPromise:true}).then(()=>null,error=>error.message);await new Promise(resolve=>setTimeout(resolve,50));await first.detach();const pendingError=await pending;assert(pendingError);assert.equal(closes,1);observations.pendingAfterDetach=pendingError;
 observations.sendAfterDetach=await first.send('Runtime.enable').then(()=>null,error=>error.message);assert(observations.sendAfterDetach);observations.repeatedDetach=await first.detach().then(()=>'success',error=>error.message); // Pinned upstream may reject repeated disposal; Ferrite deliberately makes it idempotent.
 assert.equal((await second.send('Runtime.evaluate',{expression:'6*7',returnByValue:true})).result.value,42);assert.equal(await page.evaluate(()=>6*7),42);assert.equal(await other.evaluate(()=>6*7),42);const root=await browser.newBrowserCDPSession();assert((await root.send('Browser.getVersion')).product);observations.isolation={otherSession:true,page:true,otherPage:true,browser:true};
 let secondClosed=0;second.on('close',()=>secondClosed++);await page.close();assert.equal(secondClosed,1);observations.pageDisposalClosesSession=true;observations.eventsBeforeDetach=events;observations.note='Public Playwright target sessions. Ferrite adds Rust clone/last-owner disposal, idempotent detach, explicit event lag and caller operation controls.';
 await writeFile(new URL('cdp-session-reference.json',import.meta.url),JSON.stringify(observations,null,2)+'\n');
}finally{await browser.close();}
console.log('Passed pinned target-session detach, pending-call settlement, event and transport isolation.');
