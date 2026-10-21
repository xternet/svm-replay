#!/usr/bin/env node
import {spawn} from "node:child_process";
import {activeBinary} from "./installation.mjs";
// Retain the old read-only datasource adapter only for explicit legacy start/stop.
// The canonical `fork --after` and its help belong to the native historical engine.
if(process.argv[2]==="fork"&&["start","stop"].includes(process.argv[3]))await import("./fork.mjs");
else try {
    const binary=await activeBinary();
    const args=process.argv.slice(2);
    if(process.platform==="linux"&&["fork","session"].includes(args[0])&&
        !args.some(arg=>arg==="--parent-pid"||arg.startsWith("--parent-pid=")))args.push("--parent-pid",String(process.pid));
    const child=spawn(binary,args,{stdio:"inherit",shell:false});
    // The native CLI owns cleanup. The launcher waits for it, including signals.
    const interrupt=()=>child.kill("SIGINT"), terminate=()=>child.kill("SIGTERM");
    process.on("SIGINT",interrupt);process.on("SIGTERM",terminate);
    child.once("error",error=>{console.error(`SVM_REPLAY_LAUNCH: ${error.message}`);process.exitCode=1;});
    child.once("close",code=>{
        process.removeListener("SIGINT",interrupt);process.removeListener("SIGTERM",terminate);
        process.exitCode=code===null?1:code;
    });
}catch(error){
    console.error(`SVM_REPLAY_LAUNCH: ${error.message}\nInstall and activate a pinned native bundle with svm-replay-install --activate. See installation guide.`);
    process.exitCode=1;
}
