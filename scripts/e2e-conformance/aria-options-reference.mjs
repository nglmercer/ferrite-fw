// Actual pinned default-mode ARIA depth, boxes, state and frame observations.
import {createRequire} from 'node:module';
import {writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';

const require=createRequire(import.meta.url);
const moduleName=process.env.FERRITE_PLAYWRIGHT_MODULE||'playwright';
assert.equal(require(moduleName+'/package.json').version,'1.63.0');
const pw=require(moduleName),engines=[];
const html=`<!doctype html><style>
html,body{margin:0} #root{position:absolute;left:20px;top:30px;width:140px}
#child{position:absolute;left:210px;top:40px;width:160px;height:200px}
</style><section id="root" role="region" aria-label="Root">
<div><div role="group" aria-label="Nested"><h2>Heading</h2>
<button aria-pressed="true">Push</button><input type="checkbox" checked aria-label="Check">
<input type="checkbox" id="mixed" aria-label="Mixed"><button disabled>Disabled</button>
<div role="button" aria-expanded="false" aria-selected="true" aria-label="Flags"></div>
<div role="group" aria-label="Hidden" style="display:none"><button>Hidden child</button></div>
</div></div><div id="host"></div></section>
<iframe id="child" srcdoc="<style>body{margin:0}#group{position:absolute;left:20px;top:40px;width:100px;height:60px}</style><div id='group' role='group' aria-label='Frame'><button>Frame button</button></div>"></iframe>`;
function flatten(tree){return tree.flatMap(node=>node&&typeof node==='object'?[node,...flatten(node.children||[])]:[]); }
for(const name of ['chromium','firefox']){
 const launch=name==='chromium'?{executablePath:process.env.FERRITE_CHROMIUM_PATH}:{channel:'moz-firefox',executablePath:process.env.FERRITE_FIREFOX_PATH||'/usr/bin/firefox'};
 const browser=await pw[name].launch({...launch,timeout:30000});
 try{
  const context=await browser.newContext({viewport:{width:400,height:300}}),page=await context.newPage(),cases=[];
  try{
   await page.setContent(html);
   await page.evaluate(()=>{
    document.querySelector('#mixed').indeterminate=true;
    document.querySelector('#host').attachShadow({mode:'open'}).innerHTML='<div role="group" aria-label="Shadow"><label for="edit">Edit</label><textarea id="edit" disabled></textarea></div>';
   });
   const root=page.locator('#root');
   const baseline=await root.ariaSnapshotJSON();
   assert.equal(baseline[0].role,'region');assert.equal(baseline[0].name,'Root');
   assert.ok(flatten(baseline).some(node=>node.name==='Shadow'));
   assert.ok(!flatten(baseline).some(node=>node.name==='Hidden'));
   assert.equal(flatten(baseline).find(node=>node.name==='Check').checked,true);
   assert.equal(flatten(baseline).find(node=>node.name==='Mixed').checked,'mixed');
   assert.ok(flatten(baseline).every(node=>!('box' in node)));
   for(const [label,options] of [
    ['default',{}],['depth-zero',{depth:0}],['depth-one',{depth:1}],
    ['depth-two',{depth:2}],['depth-four',{depth:4}],['boxes',{boxes:true}],
    ['depth-boxes',{depth:1,boxes:true}],['negative-depth',{depth:-1}],
    ['fractional-depth',{depth:1.5}],['invalid-boxes',{boxes:'yes'}],
   ]){
    const observe=async method=>{
     try{return {outcome:'passed',value:await root[method](options)}}
     catch(error){return {outcome:'caught',error:error.message.replace(/\u001b\[[0-9;]*m/g,'')}}
    };
    cases.push({name:label,options,json:await observe('ariaSnapshotJSON'),text:await observe('ariaSnapshot')});
   }
   const framed=page.frameLocator('#child').locator('#group');
   await framed.waitFor({state:'attached'});
   const frame=await framed.ariaSnapshotJSON({boxes:true});
   const domBox=await framed.evaluate(element=>{const {x,y,width,height}=element.getBoundingClientRect();return{x,y,width,height}});
   assert.deepEqual(frame[0].box,domBox);
   assert.ok(flatten(frame).some(node=>node.name==='Frame button'));
   cases.push({name:'same-origin-frame',options:{boxes:true},json:{outcome:'passed',value:frame},text:{outcome:'passed',value:await framed.ariaSnapshot({boxes:true})},domBox,locatorBox:await framed.boundingBox()});
   await page.evaluate(()=>{document.body.style.height='2000px';document.querySelector('#root').style.top='500px';window.scrollTo(0,450)});
   const scrolled=await root.ariaSnapshotJSON({boxes:true});
   const scrolledDomBox=await root.evaluate(element=>{const {x,y,width,height}=element.getBoundingClientRect();return{x,y,width,height}});
   const rounded=box=>Object.fromEntries(Object.entries(box).map(([key,value])=>[key,Math.round(value)]));
   assert.deepEqual(scrolled[0].box,rounded(scrolledDomBox));
   cases.push({name:'scrolled-viewport',options:{boxes:true},json:{outcome:'passed',value:scrolled},text:{outcome:'passed',value:await root.ariaSnapshot({boxes:true})},domBox:scrolledDomBox});
   await root.evaluate(element=>{element.style.left='20.25px';element.style.top='500.375px';element.style.width='140.625px'});
   const fractional=await root.ariaSnapshotJSON({boxes:true});
   const fractionalDomBox=await root.evaluate(element=>{const {x,y,width,height}=element.getBoundingClientRect();return{x,y,width,height}});
   assert.deepEqual(fractional[0].box,rounded(fractionalDomBox));
   cases.push({name:'fractional-boxes',options:{boxes:true},json:{outcome:'passed',value:fractional},text:{outcome:'passed',value:await root.ariaSnapshot({boxes:true})},domBox:fractionalDomBox});
   assert.equal(cases.length,13);
   engines.push({name,version:await browser.version(),cases});
  }finally{await context.close()}
 }finally{await browser.close()}
}
assert.equal(engines.length,2);
await writeFile(process.argv[2]||fileURLToPath(new URL('./aria-options-reference.json',import.meta.url)),JSON.stringify({playwright:'1.63.0',mode:'default',engines},null,2)+'\n');
console.log('Recorded 26 actual pinned ARIA option cases');
