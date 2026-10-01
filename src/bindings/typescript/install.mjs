#!/usr/bin/env node
// Explicit offline installation. No postinstall hook, credentials or implicit downloads.
import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { copyFile, cp, lstat, mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";
import { parseArgs } from "node:util";
import { activate } from "./installation.mjs";

export function selectPlatform(os = process.platform, arch = process.arch) {
    const systems = {linux:"linux", darwin:"macos", win32:"windows"};
    const cpus = {x64:"x86_64", arm64:"aarch64"};
    if (!systems[os] || !cpus[arch]) throw new Error(`unsupported platform: ${os}/${arch}`);
    return `${systems[os]}-${cpus[arch]}`;
}

function pin(value) {
    if (typeof value !== "string" || !/^[a-f0-9]{64}$/.test(value)) throw new Error("an independently verified SHA-256 is required");
    return value;
}

export async function inspectRelease(path, expected, platform) {
    pin(expected);
    if ((await lstat(path)).size > 1024 * 1024) throw new Error("release manifest exceeds 1 MiB");
    const bytes = await readFile(path);
    if (createHash("sha256").update(bytes).digest("hex") !== expected) throw new Error("release manifest integrity mismatch");
    const manifest = JSON.parse(bytes);
    if (manifest.version !== "0.1.0" || !Array.isArray(manifest.platforms)) throw new Error("unsupported release manifest");
    const matches = manifest.platforms.filter(row => row.platform === platform);
    if (matches.length !== 1) throw new Error(`platform ${platform} not included exactly once in release`);
    const row = matches[0];
    if (typeof row.file !== "string" || !/^[a-zA-Z0-9_-][a-zA-Z0-9_.-]*\.tar\.gz$/.test(row.file)) throw new Error("unsafe archive filename");
    pin(row.sha256); pin(row.bundleSha256);
    return row;
}

export function validateArchivePaths(list) {
    const roots = new Set(["bundle", "runtime-sources", "coordinator-license-additions", "LICENSE", "NOTICE"]);
    const seen = new Set();
    for (const name of list.trimEnd().split(/\r?\n/)) {
        const parts = name.replace(/\/$/, "").split("/");
        if (!/^[a-zA-Z0-9_.+/-]+$/.test(name) || parts.some(p => !p || p === "." || p === "..") || !roots.has(parts[0]) || seen.has(name)) {
            throw new Error(`unsafe archive entry: ${JSON.stringify(name)}`);
        }
        seen.add(name);
    }
}

async function sha(path) {
    const hash = createHash("sha256");
    for await (const chunk of createReadStream(path)) hash.update(chunk);
    return hash.digest("hex");
}

function run(binary, args) {
    try {
        return execFileSync(binary, args, {encoding:"utf8", timeout:120000, maxBuffer:8*1024*1024, windowsHide:true});
    } catch (error) {
        throw new Error(`${binary} failed: ${error.message}\n${error.stderr ?? ""}\n${error.stdout ?? ""}\nCheck system dependencies: Linux glibc 2.39+ and SQLite; Windows matching MSVC v14 runtime; macOS 15.`, {cause:error});
    }
}

export async function install({release, sha256, output, activate:activateCommand=false}) {
    const platform = selectPlatform();
    const manifestPath = resolve(release);
    const row = await inspectRelease(manifestPath, sha256, platform);
    if (process.platform === "linux") {
        const version = process.report.getReport().header.glibcVersionRuntime;
        if (!version || Number(version.split(".")[0]) < 2 ||
            (Number(version.split(".")[0]) === 2 && Number(version.split(".")[1]) < 39)) {
            throw new Error("Linux bundle requires glibc >=2.39 (musl/older glibc not supported)");
        }
    }
    const temp = await mkdtemp(join(tmpdir(), "svm-replay-install-"));
    const destination = resolve(output);
    try {
        const archive = join(temp, "release.tar.gz");
        // Copy first, then hash/extract the same private bytes; never execute from an unverified archive.
        await copyFile(join(dirname(manifestPath), row.file), archive);
        if (await sha(archive) !== row.sha256) throw new Error("native archive integrity mismatch");
        validateArchivePaths(run("tar", ["-tzf", archive]));
        const types = run("tar", ["-tvzf", archive]).trimEnd().split("\n");
        if (types.some(line => !/^[d-]/.test(line))) throw new Error("archive links/special files are not allowed");
        run("tar", ["-xzf", archive, "-C", temp]);
        const bundle = join(temp, "bundle", "bundle.json");
        if (await sha(bundle) !== row.bundleSha256) throw new Error("bundle manifest integrity mismatch");
        const binary = join(temp, "bundle", "bin", process.platform === "win32" ? "svm-replay.exe" : "svm-replay");
        const checked = JSON.parse(run(binary, ["doctor", "--bundle", bundle, "--bundle-sha256", row.bundleSha256]));
        if (checked.outcome !== "COMPLETED") throw new Error(`bundle doctor failed: ${JSON.stringify(checked)}`);
        const result = JSON.parse(run(binary, ["bundle", "install", "--bundle", bundle, "--bundle-sha256", row.bundleSha256, "--output", destination]));
        if (result.outcome !== "COMPLETED") throw new Error(`bundle install failed: ${JSON.stringify(result)}`);
        // Preserve redistribution sources/notices alongside the installed executables.
        for (const name of ["runtime-sources", "coordinator-license-additions", "LICENSE", "NOTICE"]) {
            await cp(join(temp, name), join(destination, name), {recursive:true, force:false, errorOnExist:true});
        }
        if(activateCommand)await activate(destination);
        return {...result, platform, next:activateCommand?"svm-replay --demo":`${result.binary} demo`};
    } finally {
        // Only the directory created by this invocation, never the user's destination.
        await rm(temp, {recursive:true});
    }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
    try {
        const {values} = parseArgs({options:{release:{type:"string"},sha256:{type:"string"},output:{type:"string"},activate:{type:"boolean"},help:{type:"boolean"}}});
        if (values.help) console.log("svm-replay-install --release release.json --sha256 <trusted manifest SHA256> --output <new directory> [--activate]\nPlace the matching native archive beside release.json. Requires Node 22+ and tar. --activate selects this install for the npm svm-replay command. No downloads or shell-profile edits.");
        else {
            if (!values.release || !values.sha256 || !values.output) throw new Error("required: --release, --sha256, --output (see --help)");
            console.log(JSON.stringify(await install(values), null, 2));
        }
    } catch (error) {
        console.error(`INSTALL_ERROR: ${error.message}`);
        process.exitCode = 1;
    }
}
