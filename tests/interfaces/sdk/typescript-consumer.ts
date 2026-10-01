// Compile this from a clean consumer directory, not underneath this workspace's
// node_modules. It validates the installed tarball's public declarations only.
import {Replay, historicalRequest, readArtifact, type Simulation} from "@xternet/svm-replay";
import {withHistoricalReplay, anchorRequest, assertCompleted} from "@xternet/svm-replay/adapters";

export async function consume(replay: Replay): Promise<Simulation> {
    const request = historicalRequest({requestId: "reviewed", family: "v3-0",
        genesisHash: "reviewed", candidate: {}, runtimeBinding: {}});
    const client = withHistoricalReplay({label: "consumer"}, replay);
    const simulation = assertCompleted(await client.simulateTransactionAt({request}));
    if (simulation.result) await readArtifact("/explicit/root", simulation.result, 1024);
    return simulation;
}
export {anchorRequest};
