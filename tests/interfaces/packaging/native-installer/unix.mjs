import {test} from "node:test";
import assert from "node:assert/strict";
import {existsSync, mkdirSync, readdirSync, readFileSync, symlinkSync, writeFileSync} from "node:fs";
import {join} from "node:path";
import {spawnSync} from "node:child_process";
import {root, fixture, run, archive, digest} from "./fixtures.mjs";

const unix={skip:process.platform==="win32"};
function refused(f,result,pattern){
    assert.equal(result.error,undefined);
    assert.notEqual(result.status,0,`${result.stdout}\n${result.stderr}`);
    assert.match(result.stderr,pattern);
    assert.equal(existsSync(f.prefix),false,"no install should have been published");
    assert.equal(existsSync(join(f.bin,"svm-replay")),false,"no launcher should have been published");
    assert.deepEqual(readdirSync(f.temp),[],"private temporary files must be removed");
}

test("native installer help needs no compiler or package manager",unix,t=>{
    const f=fixture(t),r=run(f,["--help"]);
    assert.equal(r.status,0,r.stderr);
    assert.match(r.stdout,/--archive/);
    assert.match(r.stdout,/--sha256/);
    assert.equal(existsSync(f.prefix),false);
});

test("native installer refuses unknown and incomplete options",unix,t=>{
    for(const args of [["--wat"],["--archive"],["--sha256","bad"],["--archive","missing"]]){
        const f=fixture(t);refused(f,run(f,args),/INSTALL_ERROR/);
    }
});

test("checksum mismatch cannot execute or publish the archive",unix,t=>{
    const f=fixture(t),a=archive(f);
    refused(f,run(f,["--archive",a.path,"--sha256","0".repeat(64)]),/archive integrity mismatch/);
});

test("truncated gzip is refused and temporary files are removed",unix,t=>{
    const f=fixture(t),path=join(f.directory,"broken.tar.gz");
    writeFileSync(path,"not gzip");
    refused(f,run(f,["--archive",path,"--sha256",digest(path)]),/INSTALL_ERROR/);
});

test("archive symlinks are rejected before extraction",unix,t=>{
    const f=fixture(t),source=join(f.directory,"links"),path=source+".tar.gz";
    mkdirSync(join(source,"bundle"),{recursive:true});
    symlinkSync("/",join(source,"bundle","escape"));
    assert.equal(spawnSync("tar",["-czf",path,"-C",source,"bundle"]).status,0);
    refused(f,run(f,["--archive",path,"--sha256",digest(path)]),/links\/special files/);
});

test("archive hardlinks are rejected before extraction",unix,async t=>{
    const {linkSync}=await import("node:fs");
    const f=fixture(t),a=archive(f,"hardlinks");
    linkSync(join(a.source,"bundle","README"),join(a.source,"bundle","hardlink"));
    assert.equal(spawnSync("tar",["-czf",a.path,"-C",a.source,"bundle"]).status,0);
    refused(f,run(f,["--archive",a.path,"--sha256",digest(a.path)]),/links\/special files/);
});

test("unknown archive roots are rejected",unix,t=>{
    const f=fixture(t),source=join(f.directory,"unsafe"),path=source+".tar.gz";
    mkdirSync(source);writeFileSync(join(source,"outside"),"must not be extracted");
    assert.equal(spawnSync("tar",["-czf",path,"-C",source,"outside"]).status,0);
    refused(f,run(f,["--archive",path,"--sha256",digest(path)]),/unsafe archive/);
});

test("a bundle without its pinned native CLI is refused",unix,t=>{
    const f=fixture(t),a=archive(f);
    refused(f,run(f,["--archive",a.path,"--sha256",digest(a.path)]),/missing.*(?:CLI|binary)/);
});

test("existing installations are preserved, not overwritten",unix,t=>{
    const f=fixture(t),a=archive(f);mkdirSync(f.prefix);writeFileSync(join(f.prefix,"keep"),"preserve");
    const r=run(f,["--archive",a.path,"--sha256",digest(a.path)]);
    assert.notEqual(r.status,0);assert.match(r.stderr,/already exists/);
    assert.equal(readFileSync(join(f.prefix,"keep"),"utf8"),"preserve");
});

test("an unrelated launcher or symlink is never overwritten",unix,t=>{
    for(const link of [false,true]){
        const f=fixture(t),a=archive(f);mkdirSync(f.bin);
        const target=join(f.directory,"keep");writeFileSync(target,"preserve");
        if(link)symlinkSync(target,join(f.bin,"svm-replay"));else writeFileSync(join(f.bin,"svm-replay"),"preserve");
        const r=run(f,["--archive",a.path,"--sha256",digest(a.path)]);
        assert.notEqual(r.status,0);assert.match(r.stderr,/already exists/);
        assert.equal(readFileSync(target,"utf8"),"preserve");
        assert.equal(readFileSync(join(f.bin,"svm-replay"),"utf8"),"preserve");
        assert.equal(existsSync(f.prefix),false);
    }
});

test("a real release installs, verifies all workers and executes its offline demo",{
    skip:process.platform==="win32" || !process.env.SVM_REPLAY_INSTALL_TEST_ARCHIVE,
    timeout:600000,
},t=>{
    const f=fixture(t),path=process.env.SVM_REPLAY_INSTALL_TEST_ARCHIVE;
    const pin=process.env.SVM_REPLAY_INSTALL_TEST_SHA256;
    assert.match(pin,/^[0-9a-f]{64}$/,"supply an independently recorded archive pin");
    assert.equal(digest(path),pin);
    f.prefix=join(f.directory,"native 'quoted' installation");
    const result=run(f,["--archive",path,"--sha256",pin]);
    assert.equal(result.error,undefined);assert.equal(result.status,0,result.stderr+result.stdout);
    assert.deepEqual(readdirSync(f.temp),[]);
    const launcher=join(f.bin,"svm-replay");
    for(const args of [["doctor","--json"],["demo","--example","default","--json"]]){
        const r=spawnSync(launcher,args,{encoding:"utf8",timeout:180000,maxBuffer:4*1024*1024});
        assert.equal(r.error,undefined);assert.equal(r.status,0,r.stderr+r.stdout);
        const answer=JSON.parse(r.stdout);
        assert.equal(answer.outcome,"COMPLETED");
        if(args[0]==="doctor"){
            assert.equal(answer.workers.length,7);assert.equal(answer.captureWorkers.length,7);
        }else assert.equal(answer.matched,true);
    }
    assert.ok(existsSync(join(f.prefix,"runtime-sources")),"keep the worker sources");
    assert.ok(existsSync(join(f.prefix,"NOTICE")),"keep redistribution notices");
    const repeated=run(f,["--archive",path,"--sha256",pin]);
    assert.notEqual(repeated.status,0);assert.match(repeated.stderr,/already exists/);
});
