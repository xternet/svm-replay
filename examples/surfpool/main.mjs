import { Surfnet } from "@solana/surfpool";
import { withHistoricalReplay, assertCompleted } from "@xternet/svm-replay/adapters";
import { configured } from "../shared/config.mjs";

const { replay, options } = await configured(process.argv[2]);
// An owned offline instance; its current state never supplies historical inputs.
const local = Surfnet.startWithConfig({ offline: true, blockProductionMode: "manual", airdropSol: 0 });
try {
    const surfpool = withHistoricalReplay(local, replay);
    const result = assertCompleted(await surfpool.simulateTransactionAt(options));
    console.log(JSON.stringify({ localRpc: surfpool.rpcUrl, historical: result }));
} finally {
    local.stop();
}
