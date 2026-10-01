import {test} from 'node:test';
import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
import {join} from 'node:path';
import {replaySignature} from '../dist/signature.js';

test('signature calls forward the selected bundle and retain saved admission errors',async()=>{
    const error={schema:'svm-replay-cli-error/v1',outcome:'ERROR',error:{code:'SOURCE_CONFIGURATION',message:'missing key'}};
    const bundle=['--bundle','/explicit/bundle.json','--bundle-sha256','a'.repeat(64)];
    const result=await replaySignature({tx:'1'.repeat(64)},'b'.repeat(64),undefined,async (args,timeout)=>{
        assert.equal(timeout,910000);
        assert.deepEqual(args.slice(-4),bundle);
        const savedTo=join(args[args.indexOf('--out')+1],'error.json');
        await writeFile(savedTo,JSON.stringify(error));
        return {stdout:JSON.stringify({...error,savedTo}),stderr:'',exitCode:1,signal:null};
    },bundle);
    assert.deepEqual(result.receipt,error);
    assert.equal(result.receipt.savedTo,undefined);
});
