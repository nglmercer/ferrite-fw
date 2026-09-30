import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
const require=createRequire(import.meta.url);
const modulePath=process.env.FERRITE_PLAYWRIGHT_MODULE || 'playwright';
const {request}=require(modulePath);
const version=require(modulePath+'/package.json').version;
if(version!=='1.63.0') throw Error('wrong reference version');
let resets=0, statuses=0;
const server=createServer(async (req,res)=>{
 let body='';for await(const bytes of req)body+=bytes;
 const url=new URL(req.url,'http://local');
 if(url.pathname.startsWith('/redirect/')){res.writeHead(Number(url.pathname.split('/')[2]),{location:'/echo'});res.end('redirect');return;}
 if(url.pathname==='/reset' && resets++===0){req.socket.destroy();return;}
 if(url.pathname==='/status'){statuses++;res.writeHead(503);res.end('unavailable');return;}
 if(url.pathname==='/auth'&&!req.headers.authorization){res.writeHead(401,{'www-authenticate':'Basic realm="fixture"'});res.end('challenge');return;}
 res.writeHead(200,{'content-type':'application/json'});res.end(JSON.stringify({method:req.method,body,authorization:req.headers.authorization||null}));
});await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const base=`http://127.0.0.1:${server.address().port}`;
const output={playwright:version,cases:[]};
try{
 const api=await request.newContext({baseURL:base});
 for(const method of ['POST','PUT'])for(const status of [301,302,303,307,308]){
  const response=await api.fetch(`/redirect/${status}`,{method,data:Buffer.from('abc')});
  output.cases.push({name:`${method}-${status}`,result:await response.json()});
 }
 const noRedirect=await api.get('/redirect/302',{maxRedirects:0});output.cases.push({name:'no-redirect',status:noRedirect.status(),body:await noRedirect.text()});
 const reset=await api.get('/reset',{maxRetries:1});output.cases.push({name:'retry-reset',requests:resets,status:reset.status()});
 const failed=await api.get('/status',{maxRetries:3});output.cases.push({name:'no-http-status-retry',requests:statuses,status:failed.status()});
 await api.dispose();
 for(const [name,httpCredentials]of[
  ['default',{username:'user',password:'pass'}],
  ['always',{username:'user',password:'pass',send:'always'}],
  ['origin-mismatch',{username:'user',password:'pass',send:'always',origin:'http://127.0.0.1:1'}],
 ]){
  const api=await request.newContext({baseURL:base,httpCredentials});
  output.cases.push({name:`auth-${name}-echo`,result:await(await api.get('/echo')).json()});
  const auth=await api.get('/auth');output.cases.push({name:`auth-${name}-challenge`,status:auth.status(),body:await auth.text()});
  await api.dispose();
 }
 const target=process.argv[2] || fileURLToPath(new URL('./api-reference.json',import.meta.url));
 await writeFile(target,JSON.stringify(output,null,2)+'\n');
 console.log(`Recorded ${output.cases.length} HTTP cases on Playwright ${version}`);
}finally{await new Promise(resolve=>server.close(resolve));}
