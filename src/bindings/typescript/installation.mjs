// Local command activation only. No downloads, credentials or shell-profile edits.
import {mkdir,readFile,lstat,writeFile,rename,unlink} from "node:fs/promises";
import {homedir} from "node:os";
import {join,resolve,dirname} from "node:path";
import {createHash,randomUUID} from "node:crypto";
export function activationPath(env=process.env,platform=process.platform){
    let base;
    if(platform==="win32"){
        if(!env.LOCALAPPDATA)throw new Error("LOCALAPPDATA is required");
        base=env.LOCALAPPDATA;
    }else if(platform==="darwin")base=join(homedir(),"Library","Application Support");
    else if(platform==="linux")base=env.XDG_DATA_HOME||join(homedir(),".local","share");
    else throw new Error("unsupported platform");
    return join(base,"svm-replay","command.json");
}
async function bounded(path,max){
    if((await lstat(path)).size>max)throw new Error("installation metadata exceeds bound");
    const bytes=await readFile(path);if(bytes.length>max)throw new Error("installation metadata grew beyond bound");return bytes;
}
const digest=bytes=>createHash("sha256").update(bytes).digest("hex");
export async function installedBinary(directory){
    const root=resolve(directory);
    const pin=JSON.parse(await bounded(join(root,"installation.json"),4096));
    const bytes=await bounded(join(root,"bundle.json"),1024*1024);
    if(pin.schema!=="svm-replay-installation-pin/v1"||digest(bytes)!==pin.bundleSha256)throw new Error("installation pin differs");
    const manifest=JSON.parse(bytes);
    const expected=process.platform==="win32"?"bin/svm-replay.exe":"bin/svm-replay";
    if(manifest.schema!=="svm-replay-bundle/v1"||manifest.binary!==expected||!Array.isArray(manifest.files))throw new Error("invalid installed binary path");
    const entries=manifest.files.filter(f=>f.path===expected);
    if(entries.length!==1||entries[0].executable!==true)throw new Error("missing unique CLI pin");
    const binary=join(root,expected);
    if(digest(await bounded(binary,512*1024*1024))!==entries[0].sha256)throw new Error("installed CLI hash differs");
    return binary;
}
export async function activate(directory,path=activationPath()){
    await installedBinary(directory);
    await mkdir(dirname(path),{recursive:true,mode:0o700});
    const temporary=path+"."+randomUUID();
    await writeFile(temporary,JSON.stringify({schema:"svm-replay-command/v1",directory:resolve(directory)}),{flag:"wx",mode:0o600});
    try {await rename(temporary,path);}catch(error){await unlink(temporary);throw error;}
}
export async function activeBinary(){
    const config=JSON.parse(await bounded(activationPath(),4096));
    if(config.schema!=="svm-replay-command/v1"||typeof config.directory!=="string")throw new Error("invalid command activation");
    return installedBinary(config.directory);
}
