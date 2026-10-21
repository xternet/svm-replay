import {test} from "node:test";
import assert from "node:assert/strict";
import {readFileSync, writeFileSync} from "node:fs";
import {join} from "node:path";
import {spawnSync} from "node:child_process";
import {archive, digest, fixture, root} from "./fixtures.mjs";

test("release checksums are computed from archives, not copied from claims",t=>{
    const f=fixture(t),a=archive(f),manifest=join(f.directory,"release.json"),output=join(f.directory,"SHA256SUMS");
    const file=a.path.split(/[\\/]/).pop();
    const row={platform:"linux-x86_64",file,sha256:digest(a.path)};
    const execute=()=>spawnSync(process.execPath,[join(root,"src/installer/_2_release/checksums.mjs"),manifest,output],{encoding:"utf8"});
    writeFileSync(manifest,JSON.stringify({version:"0.2.0",platforms:[row]}));
    let result=execute();assert.equal(result.status,0,result.stderr);
    const expected=`${digest(a.path)}  ${file}\n`;
    assert.equal(readFileSync(output,"utf8"),expected);
    result=execute();assert.notEqual(result.status,0,"do not overwrite an existing checksum asset");
    assert.equal(readFileSync(output,"utf8"),expected);
    writeFileSync(manifest,JSON.stringify({version:"0.2.0",platforms:[{...row,sha256:"0".repeat(64)}]}));
    result=execute();assert.notEqual(result.status,0);assert.match(result.stderr,/integrity mismatch/);
    writeFileSync(manifest,JSON.stringify({version:"0.2.0",platforms:[row,row]}));
    result=execute();assert.notEqual(result.status,0);assert.match(result.stderr,/duplicate/);
});
