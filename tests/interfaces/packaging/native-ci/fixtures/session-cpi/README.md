# Portable CPI input for native session tests

`invoke_and_ok.so` is a synthetic SBPFv0 test program built from the
[caller source](../../../../integrations/fork/battle/examples/invoke_and_ok.rs).
It invokes the supplied child program and deliberately catches its error. The
unchanged worker tests must still detect missing historical input and veto all
writes/fees; supplying the proven input must permit a real CPI commit.

This is not a captured historical transaction, account image or expected replay
result. The old private CPI fixture is not redistributed. No historical control
or worker production source is changed. `cpi_fixture.py` records the new input
explicitly and restores the worker packet's original test metadata afterward.

`build.json` pins the actual ELF, compiler and source inputs. The source adapts
Apache-2.0 Agave's caller to the existing fixture SDK. Rebuild with Anza
[platform-tools v1.54](https://github.com/anza-xyz/platform-tools/releases/tag/v1.54),
using its Rust compiler as `RUSTC` and path remapping for the checkout/registry:

```sh
cargo +1.94.0 build --locked --jobs 4 \
  --manifest-path tests/interfaces/integrations/fork/battle/Cargo.toml \
  --release --example invoke_and_ok --target sbpf-solana-solana \
  --config 'profile.release.panic="abort"' \
  --config 'profile.release.lto="fat"' --config profile.release.codegen-units=1
```

The ELF is under `target/sbpf-solana-solana/release/examples/`. Byte reproducibility
is not assumed: inspect/hash the new build and rerun the real native session
tests before updating the recorded input. Do not change historical expectations.
The input retains a source label from the public compiler's prebuilt allocator;
it contains no operator-machine path and does not open that path at runtime.
