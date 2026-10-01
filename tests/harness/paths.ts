import { resolve } from "node:path";

/** Optional corpus/migration inputs are always explicit; no machine defaults. */
export function requiredPath(name: string): string {
    const value = process.env[name];
    if (!value) throw new Error(`set ${name} to the reviewed input directory`);
    return resolve(value);
}
