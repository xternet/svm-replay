import {test} from "node:test";
import assert from "node:assert/strict";
import * as fork from "../dist/adapters/fork/index.js";
import {forkHelp} from "../dist/adapters/fork/help.js";
test("fork adapter exposes historical serving without consumer launch or alignment",()=>{
    assert.deepEqual(Object.keys(fork).sort(),["loadBoundary","serveBoundary","startHistorical"]);
});
test("TTY help colors commands and dims secondary text; plain mode has no escape codes",()=>{
    assert.match(forkHelp(true),/\x1b\[1;32mUsage:/);assert.match(forkHelp(true),/\x1b\[2m/);
    assert.doesNotMatch(forkHelp(false),/\x1b/);
});
