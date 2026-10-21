import {test} from "node:test";
import assert from "node:assert/strict";
import {existsSync, mkdirSync, symlinkSync, writeFileSync} from "node:fs";
import {join} from "node:path";
import {spawnSync} from "node:child_process";
import {archive, fixture, root} from "./fixtures.mjs";

const pwsh=process.env.SVM_REPLAY_TEST_PWSH ?? "pwsh";
const available=spawnSync(pwsh,["-NoLogo","-NoProfile","-Command","$PSVersionTable.PSVersion.ToString()"],{encoding:"utf8"}).status===0;
function invoke(f,mode,path){
    return spawnSync(pwsh,["-NoLogo","-NoProfile","-File",join(root,"tests/interfaces/packaging/native-installer/powershell.ps1"),
        "-Root",root,"-Mode",mode,"-Archive",path,"-Work",f.temp],{encoding:"utf8",timeout:30000});
}
test("PowerShell entries parse and help loads local numbered owners",{skip:!available},t=>{
    const f=fixture(t),r=invoke(f,"parse","");
    assert.equal(r.status,0,r.stderr);assert.match(r.stdout,/-Archive.*-Sha256/);
});
test("Windows launcher preserves spaces, Unicode profiles and literal percent paths",{skip:!available},t=>{
    const f=fixture(t),r=invoke(f,"launcher","");assert.equal(r.status,0,r.stderr);
});
test("PowerShell extracts a safe regular-file archive without executing it",{skip:!available},t=>{
    const f=fixture(t),a=archive(f),r=invoke(f,"extract",a.path);
    assert.equal(r.status,0,r.stderr);assert.ok(existsSync(join(f.temp,"payload/bundle/README")));
});
test("PowerShell refuses an archive missing its native CLI",{skip:!available},t=>{
    const f=fixture(t),a=archive(f),r=invoke(f,"missing",a.path);
    assert.notEqual(r.status,0);assert.match(r.stderr,/missing.*CLI/);
    assert.equal(existsSync(join(f.temp,"installed")),false);
});
test("PowerShell refuses a corrupt archive",{skip:!available},t=>{
    const f=fixture(t),path=join(f.directory,"corrupt.tar.gz");writeFileSync(path,"not gzip");
    const r=invoke(f,"extract",path);assert.notEqual(r.status,0);assert.match(r.stderr,/invalid archive/);
});
test("PowerShell refuses unknown archive roots before extraction",{skip:!available},t=>{
    const f=fixture(t),source=join(f.directory,"unsafe"),path=source+".tar.gz";
    mkdirSync(source);writeFileSync(join(source,"outside"),"do not extract");
    assert.equal(spawnSync("tar",["-czf",path,"-C",source,"outside"]).status,0);
    const r=invoke(f,"extract",path);assert.notEqual(r.status,0);assert.match(r.stderr,/unsafe archive/);
    assert.equal(existsSync(join(f.temp,"payload")),false);
});
test("PowerShell refuses symbolic-link archive entries",{skip:!available || process.platform==="win32"},t=>{
    const f=fixture(t),a=archive(f);symlinkSync("/",join(a.source,"bundle/escape"));
    assert.equal(spawnSync("tar",["-czf",a.path,"-C",a.source,"bundle"]).status,0);
    const r=invoke(f,"extract",a.path);assert.notEqual(r.status,0);assert.match(r.stderr,/links\/special files/);
    assert.equal(existsSync(join(f.temp,"payload")),false);
});
