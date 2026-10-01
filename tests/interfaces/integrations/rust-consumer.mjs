import assert from "node:assert/strict";
import {spawn} from "node:child_process";
import {mkdir,readFile,writeFile,copyFile} from "node:fs/promises";
import {resolve,join,dirname} from "node:path";
import {fileURLToPath} from "node:url";
import {createHash} from "node:crypto";
const [bundle,prepared,priorReceipt,outArg]=process.argv.slice(2);
if(!outArg)throw new Error("usage: node rust-consumer.mjs INSTALLED_BUNDLE PREPARED_REQUEST PRIOR_RECEIPT NEW_OUT");
const out=resolve(outArg),root=resolve(dirname(fileURLToPath(import.meta.url)),"../../..");await mkdir(out,{mode:0o700});
const hash=b=>createHash("sha256").update(b).digest("hex");
const manifest=JSON.parse(await readFile(bundle,"utf8"));
const catalogPath=join(dirname(resolve(bundle)),manifest.catalog);
const catalogPin=manifest.files.find(f=>f.path===manifest.catalog).sha256;
assert.equal(hash(await readFile(catalogPath)),catalogPin);
const cargo=`[package]\nname = "m18-external-rust-consumer"\nversion = "0.1.0"\nedition = "2021"\npublish = false\n[dependencies]\nsvm-replay-engine = { path = ${JSON.stringify(join(root,"src/engine"))} }\nsvm-replay-protocol = { path = ${JSON.stringify(join(root,"src/protocol"))} }\n[[bin]]\nname = "m18-external-rust-consumer"\npath = "main.rs"\n`;
await writeFile(join(out,"Cargo.toml"),cargo,{flag:"wx",mode:0o600});await copyFile(join(root,"examples/rust/main.rs"),join(out,"main.rs"));
async function run(name,bin,args){
    const child=spawn(bin,args,{cwd:out,stdio:["ignore","pipe","pipe"],env:{...process.env,CARGO_TARGET_DIR:join(root,"target")}});
    const stdout=[],stderr=[];child.stdout.on("data",b=>stdout.push(b));child.stderr.on("data",b=>stderr.push(b));
    const code=await new Promise((done,fail)=>{child.once("error",fail);child.once("close",done);});
    const text=Buffer.concat(stdout).toString();await writeFile(join(out,`${name}.stdout`),text,{flag:"wx"});
    await writeFile(join(out,`${name}.stderr`),Buffer.concat(stderr),{flag:"wx"});
    assert.equal(code,0,Buffer.concat(stderr).toString());return text;
}
const start=performance.now();await run("build","cargo",["build","--offline","--manifest-path",join(out,"Cargo.toml"),"-j","4"]);
const built=join(root,"target/debug/m18-external-rust-consumer"),binary=join(out,"rust-consumer");await copyFile(built,binary);
const receipt=JSON.parse(await run("run",binary,[resolve(prepared),catalogPath,catalogPin,join(dirname(resolve(bundle)),manifest.binary),join(out,"data")]));
assert.equal(receipt.outcome,"COMPLETED");assert.equal(receipt.implementationSha256,hash(await readFile(binary)));
assert.equal(receipt.processOwnerSha256,manifest.files.find(f=>f.path===manifest.binary).sha256);
assert.notEqual(receipt.implementationSha256,receipt.processOwnerSha256);
const previous=JSON.parse(await readFile(priorReceipt,"utf8"));assert.equal(receipt.output.sha256,previous.output.sha256);
const output=await readFile(join(dirname(receipt.receiptPath),receipt.output.file));assert.equal(hash(output),receipt.output.sha256);
const summary={status:"PASS",receiptPath:receipt.receiptPath,hostSha256:receipt.implementationSha256,
    ownerSha256:receipt.processOwnerSha256,originalOutputUnchanged:true,elapsedMs:performance.now()-start};
await writeFile(join(out,"summary.json"),JSON.stringify(summary),{flag:"wx"});console.log(JSON.stringify(summary));
