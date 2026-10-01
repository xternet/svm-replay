#!/usr/bin/env node
import {spawn} from "node:child_process";
import {activeBinary} from "./installation.mjs";
try {
    const binary=await activeBinary();
    const child=spawn(binary,process.argv.slice(2),{stdio:"inherit",shell:false});
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
