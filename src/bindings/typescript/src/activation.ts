import {readFile,lstat} from "node:fs/promises";
import {homedir} from "node:os";
import {join} from "node:path";
import {ReplayError} from "./types.js";

export function activationPath(env:NodeJS.ProcessEnv=process.env,platform:string=process.platform):string {
    let base:string;
    if(platform==="win32"){
        if(!env.LOCALAPPDATA)throw new ReplayError("NOT_INSTALLED","LOCALAPPDATA is required");
        base=env.LOCALAPPDATA;
    }else if(platform==="darwin")base=join(homedir(),"Library","Application Support");
    else if(platform==="linux")base=env.XDG_DATA_HOME||join(homedir(),".local","share");
    else throw new ReplayError("UNSUPPORTED_PLATFORM","unsupported platform");
    return join(base,"svm-replay","command.json");
}

export async function activeDirectory():Promise<string> {
    const path=activationPath();
    let bytes:Buffer;
    try {
        if((await lstat(path)).size>4096)throw new ReplayError("BUNDLE_FORMAT","activation metadata exceeds bound");
        bytes=await readFile(path);
    }catch(error){
        if((error as NodeJS.ErrnoException).code==="ENOENT")throw new ReplayError("NOT_INSTALLED","No active SVM Replay bundle; install and activate a native bundle first");
        throw error;
    }
    if(bytes.length>4096)throw new ReplayError("BUNDLE_FORMAT","activation metadata exceeds bound");
    let value:unknown;
    try {value=JSON.parse(bytes.toString("utf8"));}
    catch {throw new ReplayError("BUNDLE_FORMAT","invalid command activation JSON");}
    if(value===null||typeof value!=="object"||!("schema" in value)||value.schema!=="svm-replay-command/v1"
        ||!("directory" in value)||typeof value.directory!=="string"||value.directory.length===0)
        throw new ReplayError("BUNDLE_FORMAT","invalid command activation");
    return value.directory;
}
