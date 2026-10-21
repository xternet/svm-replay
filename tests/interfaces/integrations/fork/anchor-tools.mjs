/** Workspace setup owned by the opt-in Anchor drivers; Anchor owns Surfpool deployment. */
import {spawnSync,execFileSync} from "node:child_process";
import {readFile,mkdir,readdir} from "node:fs/promises";
import {join,resolve} from "node:path";
import {freePort,closed,SUPPORT_ACCOUNTS} from "./shared.mjs";

/** Anchor owns Surfpool startup; its hooks start and stop only the historical server. */
export function anchorConfig({signature,dataDir,alchemyConfig,wallet,matchDatasource,port=8999,rpcPort=8899,wsPort=8900}){
    const quote=s=>`'${String(s).replaceAll("'","'\\''")}'`;
    const start=`svm-replay fork start ${signature} --detach --exit-with $PPID --port ${port} --allow-new-accounts --data-dir ${quote(resolve(dataDir))} --alchemy-config ${quote(resolve(alchemyConfig))} ${SUPPORT_ACCOUNTS.map(key=>`--also ${key}`).join(" ")}`;
    return `[surfpool]\nonline = true\nrpc_port = ${rpcPort}\nws_port = ${wsPort}\ndatasource_rpc_url = "http://127.0.0.1:${port}"\nairdrop_addresses = ${JSON.stringify([wallet])}\n`+
        `extra_args = ${JSON.stringify([...(matchDatasource?["--match-datasource"]:[]),"--skip-blockhash-check"])}\n\n`+
        `[hooks]\npre_test = ${JSON.stringify(start)}\npost_test = "svm-replay fork stop --port ${port}"\n`;
}

export async function anchorPorts(){
    const port=await freePort();
    // Surfpool's current Anchor deployment runbook sends to its default RPC port.
    if(!await closed(8899)||!await closed(8900))throw new Error("Anchor's default RPC/websocket ports are occupied; retry after their owner exits");
    return {port,rpcPort:8899,wsPort:8900};
}

/** Fresh copied workspaces have no deploy keypairs yet; generate them before Anchor synchronizes ids. */
export async function prepareProgramKeys(directory,env){
    const toml=await readFile(join(directory,"Anchor.toml"),"utf8");
    const section=/^\[programs\.localnet\]\s*$([\s\S]*?)(?=^\[|$(?![\s\S]))/m.exec(toml);
    if(section===null)throw new Error(`no localnet programs in ${directory}/Anchor.toml`);
    await mkdir(join(directory,"target/deploy"),{recursive:true});
    for(const match of section[1].matchAll(/^([A-Za-z0-9_-]+)\s*=/gm)){
        const path=join(directory,"target/deploy",`${match[1]}-keypair.json`);
        try {await readFile(path);}
        catch(error){if(error.code!=="ENOENT")throw error;
            execFileSync("solana-keygen",["new","--no-bip39-passphrase","-s","-o",path],{cwd:directory,env,stdio:"pipe"});}
    }
}

/** Separate SBF-only arguments from IDL's cargo-test arguments; both outputs belong to this workspace. */
export async function buildAnchor(directory,env,arch){
    const run=args=>spawnSync("anchor",args,{cwd:directory,env,encoding:"utf8",timeout:1800000});
    const built=run(["build","--no-idl","--skip-lint","--arch",arch,"--","--sbf-out-dir",join(directory,"target/deploy")]);
    if(built.status!==0)return built;
    let stdout=built.stdout,stderr=built.stderr;
    await mkdir(join(directory,"target/idl"),{recursive:true});await mkdir(join(directory,"target/types"),{recursive:true});
    for(const file of (await readdir(join(directory,"target/deploy"))).filter(name=>name.endsWith(".so"))){
        const name=file.slice(0,-3),idl=run(["idl","build","--skip-lint","--program-name",name,
            "--out",join(directory,"target/idl",`${name}.json`),"--out-ts",join(directory,"target/types",`${name}.ts`)]);
        stdout+=idl.stdout;stderr+=idl.stderr;
        if(idl.status!==0)return {...idl,stdout,stderr};
    }
    return {...built,stdout,stderr};
}
