const assert = require('node:assert/strict');
const fs = require('node:fs');
const anchor = require('@anchor-lang/core');
const { HistoricalFork } = require('./tests/historical.js');

(async () => {
  const hist = process.env.M91_MODE === 'historical';
  let fork;
  const rpc = async (method, params = []) => hist ? fork.rpc(method, params) :
    (await (await fetch(process.env.ANCHOR_PROVIDER_URL, {method:'POST',
      headers:{'Content-Type':'application/json'}, body:JSON.stringify({jsonrpc:'2.0', id:1, method, params})})).json()).result;
  try {
    if (hist) fork = await HistoricalFork.start({after:'413023260'});
    const keypair = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync('wallet.json'))));
    const provider = hist ? fork.provider(keypair) : new anchor.AnchorProvider(
      new anchor.web3.Connection(process.env.ANCHOR_PROVIDER_URL, 'processed'),
      new anchor.Wallet(keypair), {commitment:'processed', preflightCommitment:'processed'});
    const slot = await rpc('getSlot', [{commitment:'processed'}]);
    for (const [pubkey, expected] of Object.entries(JSON.parse(fs.readFileSync('expected.json')))) {
      const actual = (await rpc('getAccountInfo', [pubkey, {encoding:'base64', commitment:'processed'}])).value;
      assert(actual, pubkey);
      for (const field of ['owner','executable','lamports']) assert.equal(actual[field], expected[field], field);
      assert.equal(actual.data[0], expected.data[0]);
    }
    const idl = JSON.parse(fs.readFileSync('target/idl/demo.json'));
    const program = new anchor.Program(idl, provider);
    if (process.env.M91_DEPLOY !== 'true') {
    const loader = new anchor.web3.PublicKey('BPFLoaderUpgradeab1e11111111111111111111111');
    const [dataAddress] = anchor.web3.PublicKey.findProgramAddressSync([program.programId.toBuffer()], loader);
    const dataAccount = (await rpc('getAccountInfo', [dataAddress.toBase58(), {encoding:'base64', commitment:'processed'}])).value;
    assert(dataAccount);
    const data = Buffer.from(dataAccount.data[0], 'base64');
    assert.equal(data.readUInt32LE(0), 3);
    assert.equal(data.readBigUInt64LE(4), 0n);
    const authority = new anchor.web3.PublicKey(data.subarray(13,45)).toBase58();
    assert.equal(authority, process.env.M91_UPGRADEABLE === 'true' ? keypair.publicKey.toBase58() : anchor.web3.PublicKey.default.toBase58());
    }
    const [counter] = anchor.web3.PublicKey.findProgramAddressSync([Buffer.from('counter')], program.programId);
    if (hist) await fork.setAccount(counter);
    // First invocation must succeed without an explicit slot advance.
    const first = await program.methods.initialize().rpc();
    await program.methods.increment().rpc();
    const status = await rpc('getSignatureStatuses', [[first]]);
    assert.equal(status.value[0].err, null);
    if (hist) assert.equal(await rpc('getSlot', [{commitment:'processed'}]), slot);
    console.log('M91_FIXTURES_OK', JSON.stringify({mode:process.env.M91_MODE,
      upgradeable:process.env.M91_UPGRADEABLE, firstInvocationWithoutAdvance:true,
      fixtureCount:Object.keys(JSON.parse(fs.readFileSync('expected.json'))).length, slot}));
  } finally {
    if (fork) await fork.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
