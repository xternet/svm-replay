import { configured } from "../shared/config.mjs";
import { assertCompleted } from "@xternet/svm-replay/adapters";

const { replay, options } = await configured(process.argv[2]);
const result = assertCompleted(await ("tx" in options ? replay.replay(options) : replay.simulate(options)));
console.log(JSON.stringify(result));
