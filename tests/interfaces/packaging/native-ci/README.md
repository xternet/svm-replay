# Native candidate qualification

This manual lane builds the checked-out commit on six standard public runners.
It executes one retained historical case per runtime family, reference/off/capture
equality, trace/debug and SDK checks, native session tests and clean installation.
It does not qualify whole historical intervals or replace the private corpus.
The build job rechecks the checkout and pinned worker sources before archiving.
A separate six-target job downloads those exact artifacts, installs without
building any runtime and exercises a fresh packed SDK consumer. Windows checks
both PowerShell 7 (`pwsh`) and the stock Windows PowerShell (`powershell.exe`).
These workflow definitions are not proof that the six hosted targets have run.

The input manifest pins source, licenses and a small historical qualification
selection by URL, byte length and SHA-256. No provider credentials are used.
The proposed validation-inputs-0.2.0 assets require separate publication approval;
they are test inputs, not an official runtime or npm/crates release. Missing URLs
or bad digests fail before execution. No alternate provider or retry is used.

After those inputs and the validation branch have been approved and published:

```sh
gh workflow run ci.yml --ref 0.2.0 -f native=true
```

Ordinary CI remains separate. The native lane uploads diagnostics and the exact
tested archives/SDK tarballs for one day; it never publishes a release or registry
package. The source checkout, native host/compiler/headers, fourteen workers,
original outputs, capture gates and fresh installed demos must all qualify.

New native builds remap checkout and toolchain path prefixes to neutral names.
This does not rewrite historical account images or third-party embedded ELFs,
clear redistribution rights, or qualify previously built binaries retroactively.
The worker source export uses neutral path labels for retained build records and
legacy fixture references. Those legacy tests still need private inputs and are
not claimed as passing here. New worker bindings identify the actual new source
inventory; retained binary/source pins describe only the original qualification.
Five immutable legacy probe ELFs still contain builder paths; their disclosure
also needs review before publishing the input archive. They are not redacted.
