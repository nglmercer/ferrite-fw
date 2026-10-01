// Pinned real PDFs, inspected with Poppler; no PDF binary byte-for-byte comparisons.
import {createRequire} from 'node:module';
import {mkdtemp, writeFile, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawnSync} from 'node:child_process';
import assert from 'node:assert/strict';
const require = createRequire(import.meta.url);
const moduleName = process.env.FERRITE_PLAYWRIGHT_MODULE || 'playwright';
assert.equal(require(moduleName + '/package.json').version, '1.63.0');
const {chromium} = require(moduleName);
const root = await mkdtemp(join(tmpdir(), 'ferrite-pdf-reference-'));
const command = (binary,args) => {const result=spawnSync(binary,args,{env:{...process.env,LC_ALL:'C'}});assert.equal(result.status,0,result.stderr.toString());return result.stdout;};
const content = `<!doctype html><title>Ferrite print</title><style>html,body{margin:0;padding:0;font-family:Arial,sans-serif}#swatch{width:96px;height:48px;background:rgb(20,80,210)}h1{font-size:20px;margin:16px 0 10px}p{font-size:12px}.print{display:none}@media print{.print{display:block}.screen{display:none}}</style><div id=swatch></div><h1>Ferrite PDF</h1><p>Native content</p><p class=print>PrintMarker</p><p class=screen>ScreenMarker</p>`;
const cases = [];
let browser;
try {
  browser = await chromium.launch({executablePath:process.env.FERRITE_CHROMIUM_PATH,timeout:30000});
  const page=await browser.newPage();await page.setContent(content);
  async function capture(name,options,width,height) {
    const path=join(root,name+'.pdf');await page.pdf({...options,path});
    const info=command('pdfinfo',[path]).toString();
    const dimensions=info.match(/Page size:\s*([\d.]+) x ([\d.]+)/);
    const pages=Number(info.match(/Pages:\s*(\d+)/)[1]);assert.equal(pages,1);
    assert.ok(Math.abs(Number(dimensions[1])-width)<1);assert.ok(Math.abs(Number(dimensions[2])-height)<1);
    const text=command('pdftotext',[path,'-']).toString();
    const record={name,width:Number(dimensions[1]),height:Number(dimensions[2]),pages,text};
    cases.push(record);return {path,record};
  }
  async function pixel(path,x,y) {
    const stem=path.slice(0,-4);command('pdftoppm',['-f','1','-singlefile','-r','72',path,stem]);
    const ppm=await readFile(stem+'.ppm'), header=ppm.subarray(0,64).toString('ascii').match(/^P6\s+(\d+)\s+(\d+)\s+255\s/);
    assert.ok(header);const position=header[0].length+(y*Number(header[1])+x)*3;return [...ppm.subarray(position,position+3)];
  }
  const first=await capture('default',{},612,792);assert.ok(first.record.text.includes('PrintMarker'));assert.ok(!first.record.text.includes('ScreenMarker'));
  const header=await capture('a4-header',{format:'A4',landscape:true,margin:{top:'12.7mm',right:'12.7mm',bottom:'12.7mm',left:'12.7mm'},printBackground:true,tagged:true,outline:true,displayHeaderFooter:true,headerTemplate:"<div style='font-size:10px;width:100%;text-align:center'>HeaderMarker</div>",footerTemplate:"<div style='font-size:10px;width:100%;text-align:center'>FooterMarker <span class='pageNumber'></span>/<span class='totalPages'></span></div>"},842.4,595.44);
  assert.ok(header.record.text.includes('HeaderMarker') && header.record.text.includes('FooterMarker') && header.record.text.includes('1/1'));
  assert.ok((await readFile(header.path)).includes(Buffer.from('/Outlines')));
  const custom={width:'384px',height:'15.24cm'};
  const off=await capture('background-off',custom,288,432),on=await capture('background-on',{...custom,printBackground:true},288,432),half=await capture('scaled-half',{...custom,printBackground:true,scale:0.5},288,432);
  off.record.pixel=await pixel(off.path,20,10);on.record.pixel=await pixel(on.path,20,10);half.record.pixel=await pixel(half.path,50,10);
  assert.ok(off.record.pixel.every(n=>n>245));assert.ok(on.record.pixel[2]>180 && on.record.pixel[0]<40);assert.ok(half.record.pixel.every(n=>n>245));
  await page.setContent(content+'<style>@page{size:3in 5in;margin:0}</style>');
  await capture('css-size',{...custom,preferCSSPageSize:true},216,360);await capture('css-fit',custom,288,432);
  await page.setContent("<style>body{margin:0;font:18px Arial}section{break-after:page;height:2in}</style><section>FirstMarker</section><section>SecondMarker</section><section style='break-after:auto'>ThirdMarker</section>");
  const range=await capture('range-second',{...custom,pageRanges:'2'},288,432);assert.ok(range.record.text.includes('SecondMarker'));assert.ok(!range.record.text.includes('FirstMarker') && !range.record.text.includes('ThirdMarker'));
  let rejected=0;
  for (const options of [{format:'Unknown'},{width:'-1in'},{scale:3},{margin:{left:'-1in'}},{pageRanges:'3-2'}]) {await assert.rejects(page.pdf(options));rejected++;}
  await writeFile(new URL('pdf-reference.json',import.meta.url),JSON.stringify({playwright:'1.63.0',engine:'chromium',cases,invalidOptionsRejected:rejected,unitNote:'Pinned JS converts cm/mm through rounded 37.8/3.78 px per unit; Ferrite uses exact 2.54 cm/25.4 mm per inch. Native paper dimensions agree within one PDF point.'},null,2)+'\n');
} finally { if(browser)await browser.close();await rm(root,{recursive:true,force:true}); }
console.log('Passed eight pinned PDF size/content/raster profiles and five invalid-option checks.');
