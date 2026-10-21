import {test} from 'node:test';
import assert from 'node:assert/strict';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {sessionArguments} from '../dist/session-options.js';
import {Replay} from '../dist/index.js';

test('literal provider credentials enter only the child environment, never argv or the parent environment',()=>{
    const name='SVM_REPLAY_SESSION_KEY';
    const previous=process.env[name];
    process.env[name]='inherited-value-must-not-win';
    try {
        const command=sessionArguments({after:'123',key:'synthetic-private-key',credentialEnv:[name]},[],undefined);
        assert.equal(command.args.includes('synthetic-private-key'),false);
        assert.equal(command.args[command.args.indexOf('--key')+1],`env:${name}`);
        assert.equal(command.env[name],'synthetic-private-key');
        assert.equal(process.env[name],'inherited-value-must-not-win');
    } finally {
        if(previous===undefined)delete process.env[name];else process.env[name]=previous;
    }
});

test('explicit credential references and key files remain references',()=>{
    process.env.SVM_TEST_KEY='synthetic-reference-value';
    try {
        const command=sessionArguments({after:'123',key:'env:SVM_TEST_KEY'},[],undefined);
        assert.equal(command.args[command.args.indexOf('--key')+1],'env:SVM_TEST_KEY');
        assert.equal(command.env.SVM_TEST_KEY,'synthetic-reference-value');
        assert.equal(command.args.includes('synthetic-reference-value'),false);
        const keyFile=join(tmpdir(),'svm-replay-provider-key');
        const file=sessionArguments({after:'123',keyFile},[],undefined);
        assert.equal(file.args.includes('--key'),false);
        assert.equal(file.args[file.args.indexOf('--key-file')+1],keyFile);
        assert.throws(()=>sessionArguments({after:'123',key:'env:bad-name'},[],undefined),e=>e.code==='INVALID_REQUEST');
    } finally {delete process.env.SVM_TEST_KEY;}
});

test('forkAt is a native session API and forwards provider settings without selecting a runtime in JS',()=>{
    assert.equal(typeof Replay.prototype.forkAt,'function');
    const command=sessionArguments({after:'tx:signature',provider:'alchemy',key:'env:API_ALCHEMY',timeoutMs:120000},['--bundle','bundle.json','--bundle-sha256','pin'],'/tmp/history');
    assert.deepEqual(command.args.slice(0,4),['fork','--after','tx:signature','--port']);
    assert.ok(command.args.includes('--provider'));
    assert.ok(command.args.includes('--key'));
    assert.ok(command.args.includes('env:API_ALCHEMY'));
    assert.ok(command.args.includes('/tmp/history'));
    assert.equal(command.timeoutMs,120000);
    assert.equal(command.args.includes('--rpc-url'),false);
    assert.throws(()=>sessionArguments({after:'slot:1',key:'x',keyFile:'key'},[],undefined),e=>e.code==='INVALID_REQUEST');
    assert.throws(()=>sessionArguments({after:'slot:1',timeoutMs:0},[],undefined),e=>e.code==='INVALID_LIMIT');
});

test('session program overrides read the ELF file exactly as the CLI payload does',async()=>{
    const {resolveProgram}=await import('../dist/session.js');
    const {mkdtemp,writeFile}=await import('node:fs/promises');
    const {tmpdir}=await import('node:os');
    const {join}=await import('node:path');
    const dir=await mkdtemp(join(tmpdir(),'svm-replay-program-'));
    const elf=Buffer.from([0x7f,0x45,0x4c,0x46,1,2,3]),text=Buffer.from('not a program');
    await writeFile(join(dir,'p.so'),elf);await writeFile(join(dir,'t.so'),text);
    const pubkey='Prog111111111111111111111111111111111111111';
    assert.deepEqual(await resolveProgram({pubkey,lamports:'5',program:`@${join(dir,'p.so')}`}),
        {pubkey,lamports:'5',owner:'BPFLoader2111111111111111111111111111111111',executable:true,dataBase64:elf.toString('base64')});
    const plain={pubkey,lamports:'1'};
    assert.equal(await resolveProgram(plain),plain);
    for(const entry of [{pubkey,program:join(dir,'p.so')},{pubkey,program:`@${join(dir,'t.so')}`},{pubkey,owner:pubkey,program:`@${join(dir,'p.so')}`}])
        await assert.rejects(resolveProgram(entry),e=>e.code==='INVALID_STATE_OVERRIDE');
});

test('a lockfile records what a fork resolved to and fails when it resolves differently',async()=>{
    const {checkLock}=await import('../dist/lockfile.js');
    const {mkdtemp,readFile,writeFile}=await import('node:fs/promises');
    const {tmpdir}=await import('node:os');
    const {join}=await import('node:path');
    const path=join(await mkdtemp(join(tmpdir(),'svm-replay-lock-')),'historical.lock.json');
    const pins={genesisHash:'net',slot:10,transactionIndex:6,signature:'sig',family:'v3-1',
        initialStateSha256:'platform-specific-not-locked'};
    await checkLock(path,'tx:sig',pins);
    assert.deepEqual(JSON.parse(await readFile(path,'utf8')).forks['tx:sig'],
        {network:'net',slot:10,transactionIndex:6,transaction:'sig',runtime:'v3-1'});
    await checkLock(path,'tx:sig',{...pins,initialStateSha256:'another build'});
    await assert.rejects(checkLock(path,'tx:sig',{...pins,family:'v4-0'}),
        e=>e.code==='LOCK_MISMATCH'&&e.message.includes('runtime'));
    await checkLock(path,'slot:9',{...pins,signature:null,transactionIndex:0});
    assert.equal(Object.keys(JSON.parse(await readFile(path,'utf8')).forks).length,2);
    await writeFile(path,'{"not":"a lock"}');
    await assert.rejects(checkLock(path,'tx:sig',pins),e=>e.code==='LOCKFILE');
});

test('concurrent initial lockfile updates preserve every fork in valid JSON',async()=>{
    const {checkLock}=await import('../dist/lockfile.js');
    const {mkdtemp,readFile,rm}=await import('node:fs/promises');
    const {tmpdir}=await import('node:os');
    const {join}=await import('node:path');
    const directory=await mkdtemp(join(tmpdir(),'svm-parallel-lock-'));
    try {
        const path=join(directory,'historical.lock.json');
        const selectors=Array.from({length:12},(_,index)=>`tx:parallel-${index}`);
        const results=await Promise.allSettled(selectors.map((after,index)=>checkLock(path,after,{
            genesisHash:'net',slot:10,transactionIndex:index,signature:after.slice(3),family:'v3-1',
        })));
        assert.ok(results.every(result=>result.status==='fulfilled'),JSON.stringify(results));
        const recorded=JSON.parse(await readFile(path,'utf8'));
        assert.deepEqual(Object.keys(recorded.forks).sort(),selectors.sort());
        for(const after of selectors)assert.equal(recorded.forks[after].transaction,after.slice(3));
    } finally {await rm(directory,{recursive:true});}
});

test('separate test processes serialize updates to a shared lockfile',async()=>{
    const {mkdtemp,readFile,rm}=await import('node:fs/promises');
    const {tmpdir}=await import('node:os');
    const {join}=await import('node:path');
    const {spawn}=await import('node:child_process');
    const directory=await mkdtemp(join(tmpdir(),'svm-process-lock-'));
    try {
        const path=join(directory,'historical.lock.json');
        const module=new URL('../dist/lockfile.js',import.meta.url).href;
        const source=`import {checkLock} from ${JSON.stringify(module)};
            await checkLock(process.argv[1],process.argv[2],{genesisHash:'net',slot:10,
                transactionIndex:0,signature:process.argv[2],family:'v3-1'});`;
        const selectors=Array.from({length:6},(_,index)=>`tx:process-${index}`);
        const results=await Promise.allSettled(selectors.map(after=>new Promise((resolve,reject)=>{
            const child=spawn(process.execPath,['--input-type=module','-e',source,path,after],{stdio:['ignore','ignore','pipe']});
            let stderr='';child.stderr.on('data',chunk=>{stderr+=chunk;});
            child.once('error',reject);
            child.once('close',code=>{if(code===0)resolve();else reject(new Error(`lock writer exited ${code}: ${stderr}`));});
        })));
        assert.equal(results.length,selectors.length);
        assert.ok(results.every(result=>result.status==='fulfilled'),JSON.stringify(results));
        assert.deepEqual(Object.keys(JSON.parse(await readFile(path,'utf8')).forks).sort(),selectors.sort());
    } finally {await rm(directory,{recursive:true});}
});
