import {test} from "node:test";
import assert from "node:assert/strict";
import {spawnSync} from "node:child_process";
import {fileURLToPath} from "node:url";
// Compatibility adapter help is independent of the canonical native fork CLI.
const launcher=fileURLToPath(new URL("../fork.mjs",import.meta.url));
const run=args=>spawnSync(process.execPath,[launcher,...args],{encoding:"utf8",env:{...process.env,NO_COLOR:"1",FORCE_COLOR:"0"}});
test("legacy fork adapter help uses groups, descriptions and examples without pipe colors",()=>{
    const r=run(["fork","--help"]);assert.equal(r.status,0,r.stderr);
    for(const text of ["Usage:","Commands:","Server options:","Historical state options:","Provider and cache options:","Examples:","--exit-with <PID>","--detach","--allow-new-accounts","--port <PORT>","--data-dir <DIR>"])
        assert.ok(r.stdout.includes(text),text);
    assert.doesNotMatch(r.stdout,/\x1b\[|surfpool|anchor-toml|align|new-program-formats/i);
});
test("every fork option stands on its own line, its description below and a blank line before the next",()=>{
    const lines=run(["fork","--help"]).stdout.split("\n");
    let options=0;
    lines.forEach((line,index)=>{
        const item=line.trimStart(),indent=line.length-item.length;
        if(!item.startsWith("-")||indent>6)return;
        options++;
        const previous=lines[index-1]??"",next=lines[index+1]??"";
        assert.ok(previous.trim()===""||previous.endsWith(":"),`no blank line before ${line}`);
        assert.ok(next.startsWith("          ")&&next.trim()!=="",`no description under ${line}`);
    });
    assert.ok(options>=9,`only ${options} options found`);
});
test("help fork prints the same help through the launcher without an activated engine",()=>{
    const direct=run(["fork","--help"]),alias=run(["help","fork"]);
    assert.equal(alias.status,0,alias.stderr);assert.equal(alias.stdout,direct.stdout);
});
test("pipe help stays plain even when FORCE_COLOR is set",()=>{
    const env={...process.env,FORCE_COLOR:"1"};delete env.NO_COLOR;
    const r=spawnSync(process.execPath,[launcher,"fork","--help"],{encoding:"utf8",env});
    assert.equal(r.status,0);assert.doesNotMatch(r.stdout,/\x1b/);
});
test("NO_COLOR disables help styling in a real TTY",{skip:process.platform!=="linux"},()=>{
    const r=spawnSync("script",["-q","-e","-c",`${process.execPath} ${launcher} fork --help`,"/dev/null"],
        {encoding:"utf8",env:{...process.env,NO_COLOR:"1",FORCE_COLOR:"0"}});
    assert.equal(r.status,0,r.stderr);assert.ok(r.stdout.includes("Examples:"));assert.doesNotMatch(r.stdout,/\x1b/);
});
test("removed commands and Surfpool options are rejected before preparing a fork",()=>{
    for(const args of [["surfpool"],["anchor-toml"],["align"],["start","sig","--surfpool","missing"],["start","sig","--match-datasource"],["start","sig","--new-program-formats"],["start","sig","--deploy"],["start","sig","--airdrop","wallet"]]){
        const r=run(["fork",...args]);assert.equal(r.status,1);assert.match(r.stderr,/unknown (?:command|option)/i);
    }
});
