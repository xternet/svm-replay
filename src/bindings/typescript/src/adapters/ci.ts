import {ReplayError,type Simulation} from "../types.js";

/** A completed replay may legitimately contain a failed program transaction. */
export function assertCompleted(simulation:Simulation):Simulation {
    if(simulation.receipt.outcome!=="COMPLETED"||simulation.result===undefined)
        throw new ReplayError("REPLAY_NOT_COMPLETED","CI requires a completed, verified replay",{receipt:simulation.receipt});
    return simulation;
}
