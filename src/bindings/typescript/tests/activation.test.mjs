import {test} from "node:test";
import assert from "node:assert/strict";
import {mkdtemp,mkdir,writeFile} from "node:fs/promises";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {activeDirectory,activationPath} from "../dist/activation.js";
import {Replay} from "../dist/index.js";

test("default installation is the user's activated bundle, not a project directory",async()=>{
    const names=["XDG_DATA_HOME","LOCALAPPDATA","HOME","USERPROFILE"];
    const previous=new Map(names.map(name=>[name,process.env[name]]));
    const root=await mkdtemp(join(tmpdir(),"svm-activation-"));
    for(const name of names)process.env[name]=root;
    try {
        await assert.rejects(Replay.installed(),e=>e.code==="NOT_INSTALLED");
        const record=activationPath();
        const {dirname}=await import("node:path");
        await mkdir(dirname(record),{recursive:true});
        await writeFile(record,JSON.stringify({schema:"svm-replay-command/v1",directory:"/external/bundle"}));
        assert.equal(await activeDirectory(),"/external/bundle");
        for(const value of ["{",'{}',JSON.stringify({schema:"svm-replay-command/v1",directory:""}),"x".repeat(4097)]){
            await writeFile(record,value);
            await assert.rejects(activeDirectory(),e=>e.code==="BUNDLE_FORMAT");
        }
    } finally {
        for(const [name,value] of previous){if(value===undefined)delete process.env[name];else process.env[name]=value;}
    }
});

test("explicit installation paths remain independent of activation",async()=>{
    await assert.rejects(Replay.installed(join(tmpdir(),"svm-nonexistent-explicit-bundle")),e=>e.code==="ENOENT");
});
