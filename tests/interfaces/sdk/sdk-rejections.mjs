import assert from "node:assert/strict";
import {readFile,writeFile,mkdir,copyFile} from "node:fs/promises";
import {resolve,join,dirname} from "node:path";
import {pathToFileURL} from "node:url";
const [entry,bundle,pin,request,source,sourcePin,outArg]=process.argv.slice(2);
if(!outArg)throw new Error("usage: node sdk-rejections.mjs SDK_ENTRY ONE_FAMILY_BUNDLE PIN OTHER_FAMILY_REQUEST SOURCE SOURCE_PIN NEW_OUT");
const {Replay}=await import(pathToFileURL(resolve(entry)));
const out=resolve(outArg);await mkdir(out,{mode:0o700});
const options={bundlePath:resolve(bundle),bundleSha256:pin,dataDir:join(out,"data")};
const replay=await Replay.open(options);
const absentSource=await replay.simulate({request:{path:resolve(request)}});
assert.notEqual(absentSource.receipt.outcome,"COMPLETED");assert.equal(typeof absentSource.receipt.error.code,"string");
const missingFamily=await replay.simulate({request:{path:resolve(request)},source:{path:resolve(source),sha256:sourcePin}});
assert.equal(missingFamily.receipt.outcome,"UNSUPPORTED");assert.equal(missingFamily.receipt.error.code,"UNSUPPORTED_RUNTIME");
assert.equal(missingFamily.receipt.currentStateFallback,false);
const missing=join(out,"missing-worker");await mkdir(missing);await mkdir(join(missing,"bin"));
const manifest=JSON.parse(await readFile(bundle,"utf8"));
await copyFile(bundle,join(missing,"bundle.json"));
for(const name of [manifest.binary,manifest.catalog])await copyFile(join(dirname(resolve(bundle)),name),join(missing,name));
let error;
try{await Replay.open({...options,bundlePath:join(missing,"bundle.json")});}catch(e){error=e;}
assert.equal(error?.code,"BUNDLE_IO");
const summary={status:"PASS",absentSource:absentSource.receipt.error.code,missingFamily:missingFamily.receipt.error.code,
    missingWorker:error.code,noCurrentStateFallback:true,bundleSha256:pin};
await writeFile(join(out,"summary.json"),JSON.stringify(summary),{flag:"wx",mode:0o600});console.log(JSON.stringify(summary));
