import {createHash} from "node:crypto";
import {mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync} from "node:fs";
import {tmpdir} from "node:os";
import {dirname, join, resolve} from "node:path";
import {fileURLToPath} from "node:url";
import {spawnSync} from "node:child_process";

export const root = resolve(dirname(fileURLToPath(import.meta.url)), "../../../..");
export const digest = path => createHash("sha256").update(readFileSync(path)).digest("hex");
export function fixture(t) {
    const directory = mkdtempSync(join(tmpdir(), "svm-native-install-"));
    t.after(() => rmSync(directory, {recursive:true}));
    const prefix = join(directory, "install with spaces");
    const bin = join(directory, "commands with spaces");
    const temp = join(directory, "temporary");
    mkdirSync(temp);
    return {directory,prefix,bin,temp};
}
export function run(f, args) {
    return spawnSync("sh", [join(root,"install.sh"), "--prefix",f.prefix,"--bin-dir",f.bin,...args],
        {encoding:"utf8",env:{...process.env,TMPDIR:f.temp},timeout:180000,maxBuffer:4*1024*1024});
}
export function archive(f, name="payload") {
    const source=join(f.directory,name), path=source+".tar.gz";
    mkdirSync(join(source,"bundle"),{recursive:true});
    writeFileSync(join(source,"bundle","README"),"Not an executable; invalid bundles must never run.\n");
    const result=spawnSync("tar",["-czf",path,"-C",source,"bundle"],{encoding:"utf8"});
    if(result.status!==0)throw new Error(`fixture archive: ${result.stderr}`);
    return {source,path};
}
