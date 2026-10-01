# Security

## Report privately

Email [pm@xternet.dev](mailto:pm@xternet.dev) with the subject
`SVM Replay security report`. Do not disclose vulnerabilities in public issues
or pull requests before coordinating with the maintainer.

Include the affected version/commit, impact, reproduction steps and a minimal
redacted example. Remove API keys, private payloads, account data and local paths
from requests, logs, traces and receipts. Do not send credentials or third-party
data you are not authorized to share.

## Scope and support

The current 0.1.0 release candidate is the maintenance target; no older release line
or response-time guarantee is offered. Report issues in input validation, worker
execution, artifact handling, credential exposure or replay correctness.

Use only trusted, hash-verified native worker bundles. A matching hash establishes
integrity relative to that pin, not the safety of an untrusted executable.
Keep runtime data private and do not run untrusted workers with elevated privileges.
