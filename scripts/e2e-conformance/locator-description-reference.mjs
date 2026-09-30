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
 await page.setContent('<section><button>A</button><button>B</button></section><iframe srcdoc="<button>Frame</button>"></iframe>');
 const original=page.locator('section');const labeled=original.describe('Checkout');
 const other=page.locator('section').describe('Other');
 const variants={original,labeled,replaced:labeled.describe('Replacement'),empty:labeled.describe(''),removed:labeled.describe(undefined),
  first:labeled.first(),last:labeled.last(),nth:labeled.nth(0),filter:labeled.filter({hasText:'A'}),
  visible:labeled.filter({visible:true}),scoped:labeled.locator('button'),role:labeled.getByRole('button'),
  union:labeled.or(other),intersection:labeled.and(other),all:(await labeled.all())[0],
  frameOwner:page.locator('iframe').describe('Payment frame').contentFrame().owner(),
  framePick:page.locator('iframe').describe('Payment frame').contentFrame().first().owner(),
  frameChild:page.locator('iframe').describe('Payment frame').contentFrame().locator('button')};
 const descriptions={};for(const [name,locator] of Object.entries(variants))descriptions[name]=locator.description();
 const cases=[{name:'description-chaining',result:descriptions},
  {name:'same-resolution',result:{original:await original.count(),labeled:await labeled.count(),child:await variants.scoped.count(),unchanged:original.description()===null}}];
 const error=await page.locator('#missing').describe('Checkout').click({timeout:50}).then(()=>null,error=>error.message);
 cases.push({name:'action-error',result:{operation:error.includes('locator.click'),description:error.includes('Checkout'),selector:error.includes('#missing')}});
 const output={playwright:version,engine:'chromium',browser:browser.version(),cases};
 const target=process.argv[2]||fileURLToPath(new URL('./locator-description-reference.json',import.meta.url));
 await writeFile(target,JSON.stringify(output,null,2)+'\n');
 console.log(`Recorded ${cases.length} locator description cases on Playwright ${version}, Chromium ${output.browser}`);
} finally {await browser.close();}
