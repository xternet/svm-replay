#!/usr/bin/env node
import {runForkCli} from "./dist/adapters/fork/cli.mjs";
import {activationPath} from "./installation.mjs";
runForkCli(activationPath).catch(error=>{console.error(`SVM_REPLAY_FORK: ${error.message}`);process.exit(1);});
