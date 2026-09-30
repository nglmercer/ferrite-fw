// Pinned native capture observations; PNG inspection uses only Node builtins.
import {createRequire} from 'node:module';
import {writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {inflateSync} from 'node:zlib';
import assert from 'node:assert/strict';
const require=createRequire(import.meta.url),moduleName=process.env.FERRITE_PLAYWRIGHT_MODULE||'playwright';
const {chromium,firefox}=require(moduleName),version=require(moduleName+'/package.json').version;
assert.equal(version,'1.63.0');
const html=`<style>html,body{margin:0;background:transparent}body{height:400px}#patch{position:absolute;left:40px;top:220px;width:60px;height:40px;background:rgb(255,0,0)}</style><div id=patch></div>`;
function png(bytes,points=[]) {
 const width=bytes.readUInt32BE(16),height=bytes.readUInt32BE(20),depth=bytes[24],color=bytes[25],channels=color===6?4:color===2?3:0;
 assert.equal(depth,8);assert.ok(channels,'unsupported native PNG color');assert.equal(bytes[28],0,'interlaced native PNG');
 const data=[];for(let at=8;at<bytes.length;){const n=bytes.readUInt32BE(at),type=bytes.toString('ascii',at+4,at+8);if(type==='IDAT')data.push(bytes.subarray(at+8,at+8+n));at+=12+n;}
 const raw=inflateSync(Buffer.concat(data)),stride=width*channels,rows=[];
 const paeth=(a,b,c)=>{const p=a+b-c,pa=Math.abs(p-a),pb=Math.abs(p-b),pc=Math.abs(p-c);return pa<=pb&&pa<=pc?a:pb<=pc?b:c;};
 let at=0;for(let y=0;y<height;y++){const filter=raw[at++],row=Buffer.alloc(stride),previous=rows.at(-1);assert.ok(filter<=4);for(let x=0;x<stride;x++){const a=x>=channels?row[x-channels]:0,b=previous?.[x]||0,c=x>=channels?(previous?.[x-channels]||0):0;row[x]=(raw[at++]+[0,a,b,Math.floor((a+b)/2),paeth(a,b,c)][filter])&255;}rows.push(row);}
 assert.equal(at,raw.length);
 return {width,height,pixels:points.map(([x,y])=>{assert.ok(x<width&&y<height);const row=rows[y],offset=x*channels;return [row[offset],row[offset+1],row[offset+2],channels===4?row[offset+3]:255];})};
}
const engines=[];
for(const [name,type,launch] of [
 ['chromium',chromium,{executablePath:process.env.FERRITE_CHROMIUM_PATH}],
 ['firefox-bidi',firefox,{channel:'moz-firefox',executablePath:process.env.FERRITE_FIREFOX_PATH||'/usr/bin/firefox'}],
]) {
 const browser=await type.launch({...launch,timeout:30000});
 try {
  const engine={name,version:await browser.version(),cases:[]};
  for(const dpr of [1,2]) {
   const context=await browser.newContext({viewport:{width:200,height:120},deviceScaleFactor:dpr});
   try {
    const page=await context.newPage();await page.setContent(html);await page.evaluate(()=>scrollTo(0,180));
    const original=await page.evaluate(()=>[innerWidth,innerHeight,devicePixelRatio,scrollX,scrollY]);
    const add=async(name,operation,points=[])=>{try{const bytes=await operation();engine.cases.push({name,dpr,outcome:'captured',...png(bytes,points)});}catch(error){engine.cases.push({name,dpr,outcome:'rejected',error:error.message.split('\n')[0]});}};
    await add('viewport-device',()=>page.screenshot());
    await add('viewport-css',()=>page.screenshot({scale:'css'}));
    await add('scrolled-viewport-clip',()=>page.screenshot({clip:{x:40,y:40,width:60,height:40}}),[[20,20]]);
    await add('full-document-mask',()=>page.screenshot({fullPage:true,mask:[page.locator('#patch')],maskColor:'#00ff00'}),[[60*dpr,240*dpr]]);
    await add('full-document-clip-style',()=>page.screenshot({fullPage:true,clip:{x:40,y:220,width:60,height:40},style:'#patch{background:rgb(0,0,255)!important}'}),[[20,20]]);
    await add('css-scale-document-clip',()=>page.screenshot({fullPage:true,clip:{x:40,y:220,width:60,height:40},scale:'css'}),[[20,20]]);
    await add('locator-css-scale',()=>page.locator('#patch').screenshot({scale:'css'}),[[20,20]]);
    assert.equal(await page.locator('#patch').evaluate(el=>getComputedStyle(el).backgroundColor),'rgb(255, 0, 0)');
    await page.evaluate(()=>scrollTo(0,180));
    assert.deepEqual(await page.evaluate(()=>[innerWidth,innerHeight,devicePixelRatio,scrollX,scrollY]),original);
    await add('transparent-default-background',()=>page.screenshot({omitBackground:true,clip:{x:0,y:0,width:20,height:20}}),[[5,5]]);
    await add('empty-clip',()=>page.screenshot({clip:{x:0,y:0,width:0,height:20}}));
    await add('outside-full-document-clip',()=>page.screenshot({fullPage:true,clip:{x:9999,y:9999,width:20,height:20},style:'#patch{background:blue!important}'}));
    assert.equal(await page.locator('#patch').evaluate(el=>getComputedStyle(el).backgroundColor),'rgb(255, 0, 0)');
    const zero=await page.screenshot({type:'jpeg',quality:0});assert.deepEqual([...zero.subarray(0,2)],[255,216]);engine.cases.push({name:'jpeg-quality-zero',dpr,outcome:'captured',format:'jpeg'});
    await add('nonfinite-clip-width',()=>page.screenshot({clip:{x:0,y:0,width:Infinity,height:20}}));
    if(name==='chromium'){
     const session=await context.newCDPSession(page);await session.send('Emulation.setDefaultBackgroundColorOverride',{color:{r:10,g:20,b:30,a:1}});
     await add('transparent-over-existing-background',()=>page.screenshot({omitBackground:true,clip:{x:0,y:0,width:20,height:20}}),[[5,5]]);
     await add('background-after-transparent-capture',()=>page.screenshot({clip:{x:0,y:0,width:20,height:20}}),[[5,5]]);await session.detach();
    }
   }finally{await context.close();}
  }
  for(const record of engine.cases){
   if(['scrolled-viewport-clip','full-document-mask','full-document-clip-style','css-scale-document-clip','locator-css-scale'].includes(record.name))assert.equal(record.outcome,'captured',JSON.stringify(record));
   if(['empty-clip','outside-full-document-clip'].includes(record.name))assert.equal(record.outcome,'rejected');
   if(record.name==='css-scale-document-clip'||record.name==='locator-css-scale')assert.deepEqual([record.width,record.height],[60*(name==='chromium'?1:record.dpr),40*(name==='chromium'?1:record.dpr)]);
   if(record.name==='viewport-device'||record.name==='viewport-css')assert.deepEqual([record.width,record.height],[200*(record.name==='viewport-css'&&name==='chromium'?1:record.dpr),120*(record.name==='viewport-css'&&name==='chromium'?1:record.dpr)]);
   if(record.name==='scrolled-viewport-clip')assert.deepEqual(record.pixels[0],[255,0,0,255]);
   if(record.name==='full-document-mask')assert.deepEqual(record.pixels[0],[0,255,0,255]);
   if(record.name==='full-document-clip-style')assert.deepEqual(record.pixels[0],[0,0,255,255]);
   if(record.name==='transparent-default-background')assert.equal(record.outcome,name==='chromium'?'captured':'rejected');
   if(name==='chromium'&&['transparent-default-background','transparent-over-existing-background'].includes(record.name))assert.equal(record.pixels[0][3],0);
   if(record.name==='background-after-transparent-capture')assert.deepEqual(record.pixels[0],[255,255,255,255]);
   if(record.name==='nonfinite-clip-width')assert.equal(record.outcome,'captured');
  }
  engines.push(engine);
 }finally{await browser.close();}
}
await writeFile(process.argv[2]||fileURLToPath(new URL('./screenshot-reference.json',import.meta.url)),JSON.stringify({playwright:version,engines},null,2)+'\n');
console.log(`Recorded ${engines.reduce((sum,e)=>sum+e.cases.length,0)} actual capture cases across ${engines.map(e=>e.name).join(', ')}`);
