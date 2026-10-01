import { readFile } from "node:fs/promises";
import { anchorRequest, anchorSignature, assertCompleted } from "@xternet/svm-replay/adapters";
import { configured } from "../shared/config.mjs";
import { offlineTransfer } from "./transfer.mjs";

const { config, replay } = await configured(process.argv[2]);
if (config.transfer === undefined) {
    throw new Error("supply historical payer, recipient, blockhash and lamports as transfer");
}
const { transaction, rpcCalls } = await offlineTransfer(config.transfer);
let simulation;
if (config.tx !== undefined) {
    simulation = await replay.replay(anchorSignature(config.tx, transaction));
} else {
    const input = JSON.parse(await readFile(config.requestPath, "utf8"));
    if (input.schema !== "svm-replay-historical/v1") throw new Error("Anchor example requires a historical request");
    simulation = await replay.simulate({ request: anchorRequest(input, transaction) });
}
const result = assertCompleted(simulation);
console.log(JSON.stringify({ rpcCalls, result }));
