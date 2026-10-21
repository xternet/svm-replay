//! Fork help follows the native CLI's grouped options and dimmed descriptions.
export function forkHelp(color:boolean):string{
    const style=(code:string,text:string)=>color?`\x1b[${code}m${text}\x1b[0m`:text;
    const header=(text:string)=>style("1;32",text),literal=(text:string)=>style("1;36",text),dim=(text:string)=>style("2",text);
    // Like the native help: commands on one line each; every option on its own line, its
    // description below and a blank line before the next one.
    const commands=(name:string,rows:[string,string][])=>`${header(name+":")}\n`+
        rows.map(([key,text])=>`  ${literal(key)}${" ".repeat(Math.max(2,20-key.length))}${dim(text)}`).join("\n");
    const group=(name:string,rows:[string,string][])=>`${header(name+":")}\n`+
        rows.map(([key,text])=>`${key.startsWith("--")?"      ":"  "}${literal(key)}\n          ${dim(text)}`).join("\n\n");
    return [dim("Serve the state just before a transaction as a local Solana RPC URL"),
        `${header("Usage:")} ${literal("svm-replay fork")} <COMMAND> [OPTIONS]`,
        commands("Commands",[["start <SIGNATURE>","Verify and serve the historical state"],["stop","Stop the fork and report derived state and assumptions"]]),
        group("Server options",[["--port <PORT>","Listen on localhost; 0 selects a free port [default: 8999]"],
            ["--detach","Return once the server is ready"],["--exit-with <PID>","Stop when this parent process exits"],["-h, --help","Print help"]]),
        group("Historical state options",[["--also <ADDRESS>","Read another historical account (repeatable)"],
            ["--new-account <ADDRESS>","Declare an account new and serve it as absent (repeatable)"],
            ["--allow-new-accounts","Development only: serve unknown accounts as absent and report them"]]),
        group("Provider and cache options",[["--data-dir <DIR>","Use this replay data and cache directory"],
            ["--alchemy-config <FILE>","Use this Alchemy provider configuration"]]),
        `${header("Examples:")}\n  svm-replay fork start <SIGNATURE>\n  svm-replay fork start <SIGNATURE> --detach --exit-with $PPID --allow-new-accounts\n  svm-replay fork stop`,
        dim("The Alchemy key comes from API_ALCHEMY or a .env file in the current folder.\nMore: https://github.com/xternet/svm-replay")].join("\n\n");
}
