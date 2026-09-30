// Public TestInfo.snapshotPath observations; path-only tests require no browser.
import {createRequire} from 'node:module';
import {mkdtemp, mkdir, writeFile, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, relative} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import assert from 'node:assert/strict';
const require=createRequire(import.meta.url),moduleName=process.env.FERRITE_PLAYWRIGHT_MODULE||'playwright';
const version=require(moduleName+'/package.json').version;assert.equal(version,'1.63.0');
const testModule=require.resolve(moduleName+'/test'),cli=require.resolve(moduleName+'/cli');
const root=await mkdtemp(join(tmpdir(),'ferrite-snapshot-path-reference-'));
const variants=[
 {name:'all-file-tokens',template:'{snapshotDir}{/projectName}/{testFileDir}/{testFileBaseName}/{testFileName}/{testFilePath}/{testName}/{arg}{ext}'},
 {name:'relative-platform',template:'__snapshots__{/projectName}/{platform}/{arg}{ext}'},
 {name:'unknown-browser-token',template:'{snapshotDir}/{browserName}/{arg}{ext}'},
 {name:'legacy-default'},
];
try {
 const cases=[];
 for(const variant of variants) {
  const work=join(root,variant.name),testDir=join(work,'tests');await mkdir(join(testDir,'page'),{recursive:true});
  const events=join(work,'events.jsonl');await writeFile(events,'');
  const projects=[{name:''},{name:'Desktop & tablet'},{name:'Custom',snapshotPathTemplate:'overrides/{arg}{ext}'}];
  await writeFile(join(work,'playwright.config.cjs'),`module.exports={testDir:${JSON.stringify(testDir)},snapshotDir:${JSON.stringify(join(work,'baselines'))},snapshotPathTemplate:${JSON.stringify(variant.template)},workers:1,reporter:[['json']],projects:${JSON.stringify(projects)}};`);
  await writeFile(join(testDir,'page','path.spec.cjs'),String.raw`const {test}=require(${JSON.stringify(testModule)});const fs=require('node:fs'),path=require('node:path');test.describe('suite',()=>{test('test should work',async({},info)=>{
   for(const name of ['Nested/Card.png','Body.snap','plain.png']) {
    const result=info.snapshotPath(name,{kind:name.endsWith('.png')?'screenshot':'snapshot'});
    fs.appendFileSync(${JSON.stringify(events)},JSON.stringify({variant:${JSON.stringify(variant.name)},project:info.project.name,name,path:path.relative(${JSON.stringify(work)},result).split(path.sep).join('/'),absolute:path.isAbsolute(result),created:fs.existsSync(result)})+'\n');
   }
  });});`);
  const run=spawnSync(process.execPath,[cli,'test','--config',join(work,'playwright.config.cjs')],{cwd:work,encoding:'utf8',maxBuffer:16*1024*1024,timeout:30000});
  if(run.error)throw run.error;assert.equal(run.status,0,run.stderr+'\n'+run.stdout);
  const report=JSON.parse(run.stdout);assert.equal(report.stats.expected,3);assert.equal(report.stats.unexpected,0);
  const observed=(await readFile(events,'utf8')).trim().split('\n').map(JSON.parse);assert.equal(observed.length,9);
  for(const record of observed){
   assert.equal(record.absolute,true);assert.equal(record.created,false);
   const argument=record.name.replaceAll('/','-');
   if(record.project==='Custom')assert.equal(record.path,'overrides/'+argument);
   else {
    const project=record.project?'Desktop-tablet':'';
    if(variant.name==='relative-platform')assert.equal(record.path,'__snapshots__'+(project?'/'+project:'')+'/'+process.platform+'/'+argument);
    if(variant.name==='unknown-browser-token')assert.equal(record.path,'baselines/{browserName}/'+argument);
    if(variant.name==='all-file-tokens')assert.equal(record.path,'baselines'+(project?'/'+project:'')+'/page/path.spec/path.spec.cjs/page/path.spec.cjs/suite-test-should-work/'+argument);
    if(variant.name==='legacy-default'){const at=argument.lastIndexOf('.');assert.equal(record.path,'baselines/page/path.spec.cjs-snapshots/'+argument.slice(0,at)+(project?'-'+project:'')+'-'+process.platform+argument.slice(at));}
   }
  }
  cases.push(...observed);
 }
 await writeFile(process.argv[2]||fileURLToPath(new URL('./snapshot-path-reference.json',import.meta.url)),JSON.stringify({playwright:version,platform:process.platform,cases},null,2)+'\n');
 console.log(`Recorded ${cases.length} actual public snapshot-path cases`);
}finally{await rm(root,{recursive:true,force:true});}
