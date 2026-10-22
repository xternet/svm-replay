// Exercise declared fixtures and genesis on the exact preview binaries.
import assert from 'node:assert/strict';
import{cpSync,copyFileSync,mkdirSync,readFileSync,symlinkSync,writeFileSync}from'node:fs';
import{createRequire}from'node:module';
import{spawnSync}from'node:child_process';
import{join,relative}from'node:path';
const work=process.env.TOOL_WORK,original=join(work,'toy/workspace');
const require=createRequire(join(original,'package.json'));
const anchor=require('@anchor-lang/core'),rows=[];
for(const upgradeable of [false,true]){
  const ws=join(work,'fixtures-'+upgradeable);
  cpSync(original,ws,{recursive:true,filter:p=>p===original||
    !['.anchor','.surfpool','node_modules','.upstream-anchor'].some(x=>relative(original,p).split('/').includes(x))});
  symlinkSync(join(original,'node_modules'),join(ws,'node_modules'),'dir');
  mkdirSync(join(ws,'fixtures/dir'),{recursive:true});
  copyFileSync(join(process.env.ANCHOR_SOURCE,'cli/src/historical_templates/historical_fork.js'),join(ws,'tests/historical.js'));
  copyFileSync(new URL('fixture-case.cjs',import.meta.url),join(ws,'fixture-case.cjs'));
  const image=(pubkey,data)=>({pubkey,account:{lamports:945678,owner:'11111111111111111111111111111111',
    executable:false,rentEpoch:0,data:[Buffer.from(data).toString('base64'),'base64']}});
  const explicit=anchor.web3.Keypair.generate().publicKey.toBase58();
  const a=image(anchor.web3.Keypair.generate().publicKey.toBase58(),'explicit image');
  const b=image(anchor.web3.Keypair.generate().publicKey.toBase58(),'directory image');
  writeFileSync(join(ws,'fixtures/account.json'),JSON.stringify(a));
  writeFileSync(join(ws,'fixtures/dir/account.json'),JSON.stringify(b));
  writeFileSync(join(ws,'expected.json'),JSON.stringify({[explicit]:a.account,[b.pubkey]:b.account}));
  const wallet=anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(join(ws,'wallet.json')))));
  const id=JSON.parse(readFileSync(join(ws,'target/idl/demo.json'))).address;
  let toml=readFileSync(join(ws,'Anchor.toml'),'utf8').replace(/^test = .*$/m,'test = "node fixture-case.cjs"');
  toml+=`\n[test]\nstartup_wait = 90000\n[surfpool]\nstartup_wait = 90000\nshutdown_wait = 5000\n[[test.validator.account]]\naddress = "${explicit}"\nfilename = "fixtures/account.json"\n[[test.validator.account_dir]]\ndirectory = "fixtures/dir"\n[[test.validator.fund_accounts]]\naddress = "${wallet.publicKey.toBase58()}"\nlamports = 10000000000\n[[test.genesis]]\naddress = "${id}"\nprogram = "target/deploy/demo.so"\nupgradeable = ${upgradeable}\n`;
  writeFileSync(join(ws,'Anchor.toml'),toml);
  mkdirSync(join(ws,'.surfpool/history'),{recursive:true});
  cpSync(join(work,'fixture/svm-replay-smoke-413023260/data/cache'),join(ws,'.surfpool/history/cache'),{recursive:true});
  const r=spawnSync(process.env.ANCHOR_BIN,['test','--skip-build'],{cwd:ws,encoding:'utf8',timeout:240000,maxBuffer:8<<20,
    env:{...process.env,M91_MODE:'historical',M91_UPGRADEABLE:String(upgradeable),M91_DEPLOY:'false'}});
  writeFileSync(join(work,'reports','fixtures-'+upgradeable+'.log'),String(r.stdout)+'\n'+String(r.stderr));
  if(r.error)throw r.error;assert.equal(r.status,0,String(r.stderr).slice(-3000));assert.match(r.stdout,/M91_FIXTURES_OK/);
  rows.push({upgradeable,status:'PASS',withoutSkipDeploy:true,firstInvocationWithoutAdvance:true});
}
writeFileSync(join(work,'reports/fixtures.json'),JSON.stringify({status:'PASS',rows},null,2)+'\n');
console.log(JSON.stringify({status:'PASS',cases:rows.length}));
