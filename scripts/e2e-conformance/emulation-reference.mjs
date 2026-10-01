// Pinned public media semantics, plus explicit raw-CDP metrics and UA profiles.
import {createRequire} from 'node:module';import {createServer} from 'node:http';import {writeFile} from 'node:fs/promises';import assert from 'node:assert/strict';
const require=createRequire(import.meta.url),moduleName=process.env.FERRITE_PLAYWRIGHT_MODULE||'playwright';assert.equal(require(moduleName+'/package.json').version,'1.63.0');const {chromium}=require(moduleName);
const server=createServer((request,response)=>{if(request.url==='/headers'){response.setHeader('Content-Type','application/json');response.end(JSON.stringify(request.headers));}else{response.setHeader('Content-Type','text/html');response.end("<meta name=viewport content='width=device-width'><title>Emulation</title>");}});await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));const base=`http://127.0.0.1:${server.address().port}`;
let browser;const observations={playwright:'1.63.0',engine:'chromium',media:[]};
try{
 browser=await chromium.launch({executablePath:process.env.FERRITE_CHROMIUM_PATH});const page=await browser.newPage();await page.goto(base);
 const media=()=>page.evaluate(()=>Object.fromEntries(['print','screen','(prefers-color-scheme: dark)','(prefers-color-scheme: light)','(prefers-color-scheme: no-preference)','(prefers-reduced-motion: reduce)','(forced-colors: active)','(prefers-contrast: more)'].map(q=>[q,matchMedia(q).matches])));
 const baseline=await media();observations.media.push({name:'baseline',values:baseline});
 await page.emulateMedia({media:'print',colorScheme:'dark',reducedMotion:'reduce',forcedColors:'active',contrast:'more'});const full=await media();assert.equal(full.print,true);assert.equal(full['(forced-colors: active)'],true);observations.media.push({name:'full',values:full});
 await page.emulateMedia({reducedMotion:'no-preference'});const patch=await media();assert.equal(patch.print,true);assert.equal(patch['(prefers-reduced-motion: reduce)'],false);assert.equal(patch['(forced-colors: active)'],true);observations.media.push({name:'patch',values:patch});
 await page.emulateMedia({media:null,colorScheme:null,reducedMotion:null,forcedColors:null,contrast:null});assert.deepEqual(await media(),baseline);observations.media.push({name:'reset',values:await media()});
 await page.emulateMedia({colorScheme:'no-preference'});observations.media.push({name:'no-preference',values:await media()});await page.emulateMedia({colorScheme:null});
 await page.close();
 const context=await browser.newContext({viewport:null});const nativePage=await context.newPage();await nativePage.goto(base);
 const session=await context.newCDPSession(nativePage);
 await session.send('Emulation.clearDeviceMetricsOverride');
 const metrics=()=>nativePage.evaluate(()=>({width:innerWidth,height:innerHeight,dpr:devicePixelRatio,screenWidth:screen.width,screenHeight:screen.height,x:screenX,y:screenY,type:screen.orientation.type,angle:screen.orientation.angle,touch:navigator.maxTouchPoints}));
 const metricsBaseline=await metrics();
 const profile={width:320,height:240,deviceScaleFactor:2,mobile:true,screenWidth:800,screenHeight:900,positionX:20,positionY:40,screenOrientation:{type:'landscapePrimary',angle:90},scale:1};
 await session.send('Emulation.setDeviceMetricsOverride',profile);await session.send('Emulation.setTouchEmulationEnabled',{enabled:true,maxTouchPoints:3});const custom=await metrics();assert.deepEqual(custom,{width:320,height:240,dpr:2,screenWidth:800,screenHeight:900,x:20,y:40,type:'landscape-primary',angle:90,touch:3});
 const shot=Buffer.from((await session.send('Page.captureScreenshot',{format:'png'})).data,'base64');assert.equal(shot.readUInt32BE(16),640);assert.equal(shot.readUInt32BE(20),480);
 await session.send('Emulation.setDeviceMetricsOverride',{...profile,screenOrientation:{type:'landscapePrimary',angle:37}});assert.equal((await metrics()).angle,37);
 await session.send('Emulation.clearDeviceMetricsOverride');await session.send('Emulation.setTouchEmulationEnabled',{enabled:false});await nativePage.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));assert.deepEqual(await metrics(),metricsBaseline);observations.rawCdpMetrics={custom,png:[640,480],storedPlaywrightViewport:nativePage.viewportSize(),nonQuadrantAngle:37,reset:await metrics()};
 const ua=()=>nativePage.evaluate(()=>({ua:navigator.userAgent,platform:navigator.platform,language:navigator.language}));const original=await ua();
 await session.send('Emulation.setUserAgentOverride',{userAgent:'Ferrite/7.2',acceptLanguage:'es-PE,en',platform:'FerritePlatform',userAgentMetadata:{brands:[{brand:'Ferrite',version:'7'}],fullVersionList:[{brand:'Ferrite',version:'7.2.3'}],platform:'FerriteOS',platformVersion:'1.2',architecture:'arm',model:'Phone',mobile:true,bitness:'64',wow64:false,formFactors:['Mobile']}});await nativePage.reload();
 const hints=await nativePage.evaluate(()=>navigator.userAgentData.getHighEntropyValues(['architecture','model','bitness','platformVersion','fullVersionList','wow64','formFactors']));const headers=await nativePage.evaluate(()=>fetch('/headers').then(r=>r.json()));assert.equal(headers['user-agent'],'Ferrite/7.2');assert.equal(hints.architecture,'arm');assert.equal(hints.platform,'FerriteOS');observations.rawCdpUserAgent={navigator:await ua(),hints,headers};
 await session.send('Emulation.setUserAgentOverride',{userAgent:''});await nativePage.reload();assert.deepEqual(await ua(),original);observations.rawCdpUserAgent.reset=await ua();
 observations.note='Media uses public Playwright methods. Custom screen/position/orientation/UA-metadata profiles use raw CDP and do not imply dedicated Playwright equivalents. Reset clears native metrics, rather than restoring Playwright/context viewport defaults. Raw-CDP profiles use a context with viewport:null to avoid interference from Playwright viewport bookkeeping.';
 await writeFile(new URL('emulation-reference.json',import.meta.url),JSON.stringify(observations,null,2)+'\n');
}finally{if(browser)await browser.close();await new Promise(resolve=>server.close(resolve));}
console.log('Passed pinned public media update/reset and raw-CDP device/UA profiles.');
