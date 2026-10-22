// Render the embedded Studio against real normal/historical Surfpool processes.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {once} from 'node:events';
import {createRequire} from 'node:module';
import {createServer} from 'node:net';
import {cpSync, mkdirSync, openSync, closeSync, writeFileSync} from 'node:fs';
import {join, resolve} from 'node:path';

const work = resolve(process.env.TOOL_WORK);
const require = createRequire(join(work, 'toy/workspace/package.json'));
const {Keypair, Transaction, SystemProgram} = require('@solana/web3.js');
const {chromium} = createRequire(join(process.env.PLAYWRIGHT_ROOT, 'package.json'))('playwright');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
async function until(check, description, ms = 15000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    if (await check()) return;
    await sleep(100);
  }
  throw new Error('Timed out: ' + description);
}
async function port() {
  const s = createServer(); s.listen(0, '127.0.0.1'); await once(s, 'listening');
  const result = s.address().port; s.close(); await once(s, 'close'); return result;
}
const rows = [], pageErrors = [];
const browser = await chromium.launch({headless: true, args: ['--no-sandbox']});
let child, exited, page;
async function stop(abrupt = false) {
  if (!child) return;
  child.kill(abrupt ? 'SIGKILL' : 'SIGTERM');
  await until(() => Promise.resolve(child.exitCode !== null || child.signalCode !== null), 'Surfpool shutdown');
  await exited; child = undefined;
}
let failure;
try {
  for (const historical of [false, true]) {
    const mode = historical ? 'historical' : 'normal';
    const dir = join(work, 'browser-' + mode); mkdirSync(dir);
    const rpcPort = await port(), wsPort = await port(), studioPort = await port();
    const rpcUrl = `http://127.0.0.1:${rpcPort}`;
    const studioUrl = `http://127.0.0.1:${studioPort}`;
    if (historical) cpSync(join(work, 'fixture/svm-replay-smoke-413023260/data'), join(dir, 'history'), {recursive: true});
    const rpc = async (method, params = []) => {
      const reply = await fetch(rpcUrl, {method: 'POST', headers: {'content-type':'application/json'},
        body: JSON.stringify({jsonrpc:'2.0', id:91, method, params}), signal:AbortSignal.timeout(15000)}).then(r => r.json());
      assert.equal(reply.id, 91); return reply;
    };
    const value = async (method, params) => {const reply = await rpc(method, params); assert(!reply.error, JSON.stringify(reply)); return reply.result;};
    const start = async () => {
      const args = ['start', '--host','127.0.0.1', '--port',String(rpcPort), '--ws-port',String(wsPort),
        '--studio-port',String(studioPort), '--no-tui','--no-deploy', '--airdrop-amount','0'];
      if (historical) args.push('--after','413023260','--historical-provider','alchemy',
        '--historical-data-dir',join(dir,'history'));
      else args.push('--offline');
      const log = openSync(join(dir,'server.log'),'a');
      child = spawn(process.env.SURFPOOL_BIN, args, {cwd:dir, detached:true, stdio:['ignore',log,log]});
      closeSync(log); exited = once(child, 'exit');
      await until(async () => {
        if (child.exitCode !== null) throw new Error('Surfpool exited during startup');
        try {return (await rpc('getHealth')).result === 'ok';}
        catch (error) {if (error.cause?.code === 'ECONNREFUSED') return false; throw error;}
      }, 'RPC readiness', 30000);
    };
    await start();
    page = await browser.newPage();
    let logSubscriptions = 0;
    page.on('websocket', socket => {
      const pending = new Set();
      socket.on('framesent', ({payload}) => {
        const message = JSON.parse(String(payload));
        if (message.method === 'logsSubscribe') pending.add(message.id);
      });
      socket.on('framereceived', ({payload}) => {
        const message = JSON.parse(String(payload));
        if (pending.delete(message.id)) {
          if (message.error) pageErrors.push(JSON.stringify(message.error));
          else {assert.equal(typeof message.result,'number'); logSubscriptions++;}
        }
      });
    });
    page.on('pageerror', error => pageErrors.push(String(error)));
    await page.route('**/*', route => {
      const host = new URL(route.request().url()).hostname;
      return ['127.0.0.1','localhost'].includes(host) ? route.continue() : route.abort('blockedbyclient');
    });
    await page.goto(studioUrl);
    const text = async () => page.locator('body').innerText();
    await until(async () => /Transaction Inspector/i.test(await text()), 'Studio render');
    const config = await fetch(studioUrl + '/config').then(r => r.json());
    if (historical) {
      assert.equal(config.historical.after, '413023260'); assert.equal(config.rpc_datasource_url, null);
      await until(async () => /SLOT\s+413,023,261/i.test(await text()), 'absolute historical slot');
      await until(async () => /EPOCH\s+956/i.test(await text()), 'boundary epoch');
      await sleep(500); assert.equal(await value('getSlot',[{commitment:'processed'}]), 413023261);
    } else assert.equal(config.historical, undefined);
    const transfer = async () => {
      const payer = Keypair.generate(), recipient = Keypair.generate().publicKey;
      const define = (key,lamports) => value('surfnet_setAccount',[key.toBase58(),{
        lamports,owner:SystemProgram.programId.toBase58(),data:'',executable:false,rentEpoch:0}]);
      await define(payer.publicKey,1000000000); await define(recipient,0);
      const hash = (await value('getLatestBlockhash',[{commitment:'processed'}])).value.blockhash;
      const tx = new Transaction({feePayer:payer.publicKey,recentBlockhash:hash}).add(
        SystemProgram.transfer({fromPubkey:payer.publicKey,toPubkey:recipient,lamports:1234567})); tx.sign(payer);
      const signature = await value('sendTransaction',[tx.serialize().toString('base64'),{encoding:'base64'}]);
      await until(async () => (await text()).includes(signature.slice(0,4)), 'rendered local transaction');
      assert.equal((await value('getBalance',[recipient.toBase58(),{commitment:'processed'}])).value,1234567);
      assert.equal((await value('getBalance',[payer.publicKey.toBase58(),{commitment:'processed'}])).value,998760433);
      return signature;
    };
    await until(async () => logSubscriptions > 0, 'initial logsSubscribe acknowledgement');
    await transfer(); rows.push({name:mode+'-signed-transfer-and-inspector',status:'PASS'});
    const error = await rpc('tool_nonexistent_method'); assert.equal(error.error.code,-32601);
    if (historical) {
      await value('surfnet_timeTravel',[{absoluteSlot:413424000}]);
      await until(async () => /SLOT\s+413,424,000/i.test(await text()) && /EPOCH\s+957/i.test(await text()), 'rendered epoch warp');
      await value('surfnet_resetNetwork');
      assert.equal(await value('getSlot',[{commitment:'processed'}]),413023261);
      await until(async () => /SLOT\s+413,023,261/i.test(await text()) && /EPOCH\s+956/i.test(await text()) &&
        (await text()).includes('Waiting for Transactions'), 'reset without reload');
      rows.push({name:'historical-absolute-slot-epoch-reset',status:'PASS'});
    }
    // Crash/reconnect deliberately keeps the browser open. Final normal shutdown
    // closes it first, so graceful HTTP draining is not confused with a crash.
    const previousSubscriptions = logSubscriptions;
    await stop(true); await sleep(500); await start();
    await until(async () => logSubscriptions > previousSubscriptions, 'reconnected logsSubscribe acknowledgement');
    await transfer(); rows.push({name:mode+'-restart-resubscribe-without-reload',status:'PASS'});
    await page.screenshot({path:join(work,'reports',mode+'.png')});
    await page.close(); await stop();
  }
  assert.deepEqual(pageErrors, []);
} catch (error) {
  failure = error;
  if (page && !page.isClosed()) {
    writeFileSync(join(work,'reports/failure-page.txt'), await page.locator('body').innerText());
    await page.screenshot({path:join(work,'reports/failure.png')});
  }
}
finally {
  await browser.close();
  try {await stop();} catch (error) {
    if (child) {process.kill(-child.pid,'SIGKILL'); await exited;}
    failure = failure ? new AggregateError([failure,error]) : error;
  }
}
writeFileSync(join(work,'reports/browser.json'), JSON.stringify({status:failure?'FAIL':'PASS',rows,pageErrors,
  error:failure?String(failure):null},null,2));
if (failure) throw failure;
