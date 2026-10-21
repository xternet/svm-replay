# Runtimes and versions

SVM Replay has two kinds of version:

- **SVM Replay's own version** (0.1.0, 0.2.0) numbers the tool: its CLI, SDK and native bundle.
- **Runtime versions** name the Solana validator releases whose execution rules it reproduces.
  They are always given as official Agave releases, such as `Agave 2.3.9`.

Family IDs (`v2-2` to `v4-2`) are SVM Replay's stable machine IDs for its pinned runtimes. They
stay in requests, receipts (`worker.family`) and `--family`; output for people names the runtime.
Each runtime executes through LiteSVM, the executor library, which is listed separately.
Transaction versions (legacy, v0) are unrelated to both.

## Pinned runtimes

| Runtime | Family ID | Executor | Agave release published | Reviewed mainnet slots | Installed | Transaction-qualified | Broadly qualified |
|---|---|---|---|---|---|---|---|
| Agave 2.2.20 | `v2-2` | LiteSVM 0.6.1 | 2025-07-11 | 353,951,234–363,311,999 | yes | yes | no |
| Agave 2.3.9 | `v2-3` | LiteSVM 0.7.1 | 2025-09-12 | 363,312,000–374,504,881 | yes | yes | no |
| Agave 3.0.10 | `v3-0` | LiteSVM 0.8.2 | 2025-11-04 | 374,504,882–398,988,489 | yes | yes | no |
| Agave 3.1.11 | `v3-1` | LiteSVM 0.12.0 | 2026-03-20 | 398,988,490–420,440,565 | yes | yes | no |
| Agave 4.0.0 | `v4-0` | LiteSVM 0.13.1 | 2026-05-16 | 420,440,566–429,554,363 | yes | yes | no |
| Agave 4.1.2 | `v4-1` | LiteSVM 0.14.0+pr402 | 2026-07-10 | 429,554,364–438,499,847 | yes | yes | no |
| Agave 4.2.1 | `v4-2` | LiteSVM 0.16.0 | 2026-08-13 | 438,499,848–451,646,918 | yes | yes | no |

Names come from the pinned executor provenance in the bundled runtime registry: its
`solanaProgramRuntimeVersion`, `agaveFeatureSetVersion` and `litesvmVersion` must agree with the
executor ID, or the runtime is rejected. `svm-replay capabilities` lists the installed runtimes and
every receipt carries `runtime` (`name`, `agave`, `litesvm`, `family`, `reviewedSlots`). An explicit
runtime input that is not pinned here runs with `runtime: null`: unnamed, never guessed.

### Status

- **Installed:** its worker is in the native bundle.
- **Transaction-qualified:** real mainnet transactions replayed on it matched their recorded
  results. The [qualification corpus](validation.md) covers all seven.
- **Broadly qualified:** its whole reviewed interval is qualified. None is yet. The intervals come
  from experimental adjacent-profile qualification, where equal active feature sets are necessary
  but not sufficient evidence.

### Dates and slots

- **Release published:** when Agave published the release. This is not when mainnet ran it.
- **Mainnet activation:** when validators ran a release or a feature activated. Not tracked yet (`-`).
- **Reviewed mainnet slots:** the slots for which SVM Replay selects the runtime.

A pinned build covers its reviewed slots, not its whole minor series: Agave 2.2.20 says nothing
about Agave 2.2.0 to 2.2.19. Slots newer than 451,646,918 try the newest pinned
runtime with last-known features. The receipt reports `UNQUALIFIED_NEWER_SLOT`;
original-result verification is still required. This does not qualify modified
execution or extend the reviewed interval. Slots before 353,951,234 and declared
coverage gaps remain unsupported.

## Release coverage

The release inventory has 735 Solana Labs and Agave tags, prereleases included. `-` means
not covered or not known.

| Series | Repository | Tags | First published | Pinned runtime |
|---|---|---|---|---|
| 0.1 | solana-labs/solana | 2 | 2018-02-16 | - |
| 0.2 | solana-labs/solana | 4 | 2018-02-19 | - |
| 0.3 | solana-labs/solana | 4 | 2018-03-01 | - |
| 0.4 | solana-labs/solana | 3 | 2018-03-27 | - |
| 0.5 | solana-labs/solana | 2 | 2018-04-27 | - |
| 0.6 | solana-labs/solana | 5 | 2018-05-25 | - |
| 0.7 | solana-labs/solana | 12 | 2018-06-25 | - |
| 0.8 | solana-labs/solana | 5 | 2018-09-13 | - |
| 0.9 | solana-labs/solana | 2 | 2018-09-29 | - |
| 0.10 | solana-labs/solana | 5 | 2018-11-02 | - |
| 0.11 | solana-labs/solana | 1 | 2019-01-08 | - |
| 0.12 | solana-labs/solana | 6 | 2019-03-11 | - |
| 0.13 | solana-labs/solana | 3 | 2019-04-16 | - |
| 0.14 | solana-labs/solana | 3 | 2019-04-29 | - |
| 0.15 | solana-labs/solana | 4 | 2019-05-29 | - |
| 0.16 | solana-labs/solana | 7 | 2019-06-26 | - |
| 0.17 | solana-labs/solana | 3 | 2019-07-30 | - |
| 0.18 | solana-labs/solana | 5 | 2019-08-05 | - |
| 0.19 | solana-labs/solana | 2 | 2019-09-25 | - |
| 0.20 | solana-labs/solana | 6 | 2019-10-28 | - |
| 0.21 | solana-labs/solana | 8 | 2019-11-26 | - |
| 0.22 | solana-labs/solana | 10 | 2019-12-23 | - |
| 0.23 | solana-labs/solana | 9 | 2020-01-28 | - |
| 1.0 | solana-labs/solana | 25 | 2020-02-25 | - |
| 1.1 | solana-labs/solana | 24 | 2020-03-31 | - |
| 1.2 | solana-labs/solana | 33 | 2020-05-27 | - |
| 1.3 | solana-labs/solana | 24 | 2020-08-07 | - |
| 1.4 | solana-labs/solana | 27 | 2020-10-14 | - |
| 1.5 | solana-labs/solana | 20 | 2020-12-17 | - |
| 1.6 | solana-labs/solana | 29 | 2021-03-15 | - |
| 1.7 | solana-labs/solana | 18 | 2021-06-02 | - |
| 1.8 | solana-labs/solana | 17 | 2021-10-06 | - |
| 1.9 | solana-labs/solana | 30 | 2021-12-11 | - |
| 1.10 | solana-labs/solana | 42 | 2022-03-02 | - |
| 1.11 | solana-labs/solana | 11 | 2022-06-21 | - |
| 1.13 | solana-labs/solana | 8 | 2022-09-20 | - |
| 1.14 | solana-labs/solana | 30 | 2022-09-14 | - |
| 1.15 | solana-labs/solana | 3 | 2023-02-02 | - |
| 1.16 | solana-labs/solana | 28 | 2023-05-31 | - |
| 1.17 | solana-labs/solana | 35 | 2023-10-02 | - |
| 1.17 | anza-xyz/agave | 10 | 2024-03-09 | - |
| 1.18 | solana-labs/solana | 26 | 2024-01-26 | - |
| 1.18 | anza-xyz/agave | 22 | 2024-03-09 | - |
| 2.0 | anza-xyz/agave | 26 | 2024-06-21 | - |
| 2.1 | anza-xyz/agave | 22 | 2024-10-30 | - |
| 2.2 | anza-xyz/agave | 21 | 2025-02-18 | Agave 2.2.20 |
| 2.3 | anza-xyz/agave | 14 | 2025-06-13 | Agave 2.3.9 |
| 3.0 | anza-xyz/agave | 15 | 2025-08-21 | Agave 3.0.10 |
| 3.1 | anza-xyz/agave | 15 | 2025-11-08 | Agave 3.1.11 |
| 4.0 | anza-xyz/agave | 15 | 2026-02-27 | Agave 4.0.0 |
| 4.1 | anza-xyz/agave | 9 | 2026-05-22 | Agave 4.1.2 |
| 4.2 | anza-xyz/agave | 8 | 2026-07-06 | Agave 4.2.1 |
| 4.3 | anza-xyz/agave | 10 | 2026-07-13 | - |
| 4.4 | anza-xyz/agave | 5 | 2026-08-28 | - |
| 4.5 | anza-xyz/agave | 2 | 2026-10-03 | - |

<details>
<summary>All 735 tags</summary>

| # | Official version | Repository | Published | Mainnet activation | Runtime | Reviewed mainnet slots |
|---|---|---|---|---|---|---|
| 1 | v0.1.1 | solana-labs/solana | - | - | - | - |
| 2 | v0.1.3 | solana-labs/solana | 2018-02-16 | - | - | - |
| 3 | v0.2.0 | solana-labs/solana | 2018-02-19 | - | - | - |
| 4 | v0.2.1 | solana-labs/solana | 2018-02-20 | - | - | - |
| 5 | v0.2.2 | solana-labs/solana | 2018-02-21 | - | - | - |
| 6 | v0.2.3 | solana-labs/solana | 2018-02-24 | - | - | - |
| 7 | v0.3.0 | solana-labs/solana | 2018-03-01 | - | - | - |
| 8 | v0.3.1 | solana-labs/solana | 2018-03-08 | - | - | - |
| 9 | v0.3.2 | solana-labs/solana | 2018-03-08 | - | - | - |
| 10 | v0.3.3 | solana-labs/solana | 2018-03-09 | - | - | - |
| 11 | v0.4.0-alpha | solana-labs/solana | 2018-03-27 | - | - | - |
| 12 | v0.4.0-beta | solana-labs/solana | 2018-03-29 | - | - | - |
| 13 | v0.4.0 | solana-labs/solana | 2018-03-31 | - | - | - |
| 14 | v0.5.0-beta | solana-labs/solana | 2018-04-27 | - | - | - |
| 15 | v0.5.0 | solana-labs/solana | 2018-05-01 | - | - | - |
| 16 | v0.6.0-alpha | solana-labs/solana | 2018-05-25 | - | - | - |
| 17 | v0.6.0-beta | solana-labs/solana | 2018-05-30 | - | - | - |
| 18 | v0.6.0-beta.1 | solana-labs/solana | - | - | - | - |
| 19 | v0.6.0 | solana-labs/solana | 2018-06-02 | - | - | - |
| 20 | v0.6.1 | solana-labs/solana | 2018-06-07 | - | - | - |
| 21 | v0.7.0-alpha | solana-labs/solana | 2018-06-25 | - | - | - |
| 22 | v0.7.0-beta | solana-labs/solana | 2018-07-03 | - | - | - |
| 23 | v0.7.0-rc.0 | solana-labs/solana | 2018-07-13 | - | - | - |
| 24 | v0.7.0-rc.1 | solana-labs/solana | 2018-07-13 | - | - | - |
| 25 | v0.7.0-rc.2 | solana-labs/solana | 2018-07-13 | - | - | - |
| 26 | v0.7.0-rc.3 | solana-labs/solana | 2018-07-16 | - | - | - |
| 27 | v0.7.0-rc.4 | solana-labs/solana | 2018-07-18 | - | - | - |
| 28 | v0.7.0-rc.5 | solana-labs/solana | 2018-07-18 | - | - | - |
| 29 | v0.7.0-rc.6 | solana-labs/solana | 2018-07-19 | - | - | - |
| 30 | v0.7.0 | solana-labs/solana | 2018-07-19 | - | - | - |
| 31 | v0.7.1 | solana-labs/solana | 2018-08-13 | - | - | - |
| 32 | v0.7.2 | solana-labs/solana | 2018-08-18 | - | - | - |
| 33 | v0.8.0-rc.0 | solana-labs/solana | 2018-09-13 | - | - | - |
| 34 | v0.8.0-rc.1 | solana-labs/solana | - | - | - | - |
| 35 | v0.8.0-rc.2 | solana-labs/solana | - | - | - | - |
| 36 | v0.8.0 | solana-labs/solana | 2018-09-14 | - | - | - |
| 37 | v0.8.1 | solana-labs/solana | 2018-09-17 | - | - | - |
| 38 | v0.9.0 | solana-labs/solana | 2018-09-29 | - | - | - |
| 39 | v0.9.1 | solana-labs/solana | 2018-10-22 | - | - | - |
| 40 | v0.10.0 | solana-labs/solana | 2018-11-02 | - | - | - |
| 41 | v0.10.1 | solana-labs/solana | - | - | - | - |
| 42 | v0.10.2 | solana-labs/solana | - | - | - | - |
| 43 | v0.10.3 | solana-labs/solana | - | - | - | - |
| 44 | v0.10.4 | solana-labs/solana | - | - | - | - |
| 45 | v0.11.0 | solana-labs/solana | 2019-01-08 | - | - | - |
| 46 | v0.12.0 | solana-labs/solana | 2019-03-11 | - | - | - |
| 47 | v0.12.1-pre3 | solana-labs/solana | 2019-03-23 | - | - | - |
| 48 | v0.12.1 | solana-labs/solana | 2019-03-25 | - | - | - |
| 49 | v0.12.2 | solana-labs/solana | 2019-03-27 | - | - | - |
| 50 | v0.12.3 | solana-labs/solana | 2019-03-28 | - | - | - |
| 51 | v0.12.4 | solana-labs/solana | 2019-04-03 | - | - | - |
| 52 | v0.13.0 | solana-labs/solana | 2019-04-16 | - | - | - |
| 53 | v0.13.1 | solana-labs/solana | 2019-04-24 | - | - | - |
| 54 | v0.13.2 | solana-labs/solana | 2019-04-26 | - | - | - |
| 55 | v0.14.0 | solana-labs/solana | 2019-04-29 | - | - | - |
| 56 | v0.14.1 | solana-labs/solana | 2019-05-03 | - | - | - |
| 57 | v0.14.2 | solana-labs/solana | 2019-05-11 | - | - | - |
| 58 | v0.15.0 | solana-labs/solana | 2019-05-29 | - | - | - |
| 59 | v0.15.1 | solana-labs/solana | 2019-05-31 | - | - | - |
| 60 | v0.15.2 | solana-labs/solana | 2019-06-06 | - | - | - |
| 61 | v0.15.3 | solana-labs/solana | 2019-06-07 | - | - | - |
| 62 | v0.16.0 | solana-labs/solana | 2019-06-26 | - | - | - |
| 63 | v0.16.1 | solana-labs/solana | 2019-06-27 | - | - | - |
| 64 | v0.16.2 | solana-labs/solana | 2019-07-02 | - | - | - |
| 65 | v0.16.3 | solana-labs/solana | 2019-07-12 | - | - | - |
| 66 | v0.16.4 | solana-labs/solana | 2019-07-15 | - | - | - |
| 67 | v0.16.5 | solana-labs/solana | 2019-07-17 | - | - | - |
| 68 | v0.16.6 | solana-labs/solana | 2019-07-24 | - | - | - |
| 69 | v0.17.0 | solana-labs/solana | 2019-07-30 | - | - | - |
| 70 | v0.17.1 | solana-labs/solana | 2019-08-01 | - | - | - |
| 71 | v0.17.2 | solana-labs/solana | 2019-08-21 | - | - | - |
| 72 | v0.18.0-pre0 | solana-labs/solana | 2019-08-05 | - | - | - |
| 73 | v0.18.0-pre1 | solana-labs/solana | 2019-08-17 | - | - | - |
| 74 | v0.18.0-pre2 | solana-labs/solana | 2019-08-26 | - | - | - |
| 75 | v0.18.0 | solana-labs/solana | 2019-08-27 | - | - | - |
| 76 | v0.18.1 | solana-labs/solana | 2019-09-05 | - | - | - |
| 77 | v0.19.0 | solana-labs/solana | 2019-09-25 | - | - | - |
| 78 | v0.19.1 | solana-labs/solana | 2019-10-03 | - | - | - |
| 79 | v0.20.0 | solana-labs/solana | 2019-10-28 | - | - | - |
| 80 | v0.20.1 | solana-labs/solana | 2019-10-30 | - | - | - |
| 81 | v0.20.2 | solana-labs/solana | 2019-11-04 | - | - | - |
| 82 | v0.20.3 | solana-labs/solana | 2019-11-07 | - | - | - |
| 83 | v0.20.4 | solana-labs/solana | 2019-11-10 | - | - | - |
| 84 | v0.20.5 | solana-labs/solana | 2019-11-21 | - | - | - |
| 85 | v0.21.0 | solana-labs/solana | 2019-11-26 | - | - | - |
| 86 | v0.21.1 | solana-labs/solana | 2019-12-07 | - | - | - |
| 87 | v0.21.2 | solana-labs/solana | 2019-12-09 | - | - | - |
| 88 | v0.21.3 | solana-labs/solana | 2019-12-12 | - | - | - |
| 89 | v0.21.4 | solana-labs/solana | 2019-12-13 | - | - | - |
| 90 | v0.21.5 | solana-labs/solana | 2019-12-19 | - | - | - |
| 91 | v0.21.6 | solana-labs/solana | 2019-12-24 | - | - | - |
| 92 | v0.21.7 | solana-labs/solana | 2020-01-08 | - | - | - |
| 93 | v0.22.0 | solana-labs/solana | 2019-12-23 | - | - | - |
| 94 | v0.22.1 | solana-labs/solana | 2019-12-25 | - | - | - |
| 95 | v0.22.2 | solana-labs/solana | 2020-01-06 | - | - | - |
| 96 | v0.22.3 | solana-labs/solana | 2020-01-15 | - | - | - |
| 97 | v0.22.4 | solana-labs/solana | 2020-01-25 | - | - | - |
| 98 | v0.22.5 | solana-labs/solana | 2020-01-28 | - | - | - |
| 99 | v0.22.6 | solana-labs/solana | 2020-01-30 | - | - | - |
| 100 | v0.22.7 | solana-labs/solana | 2020-02-04 | - | - | - |
| 101 | v0.22.8 | solana-labs/solana | 2020-02-15 | - | - | - |
| 102 | v0.22.9 | solana-labs/solana | 2020-02-15 | - | - | - |
| 103 | v0.23.0 | solana-labs/solana | 2020-01-28 | - | - | - |
| 104 | v0.23.1 | solana-labs/solana | 2020-02-02 | - | - | - |
| 105 | v0.23.2 | solana-labs/solana | 2020-02-04 | - | - | - |
| 106 | v0.23.3 | solana-labs/solana | 2020-02-12 | - | - | - |
| 107 | v0.23.4 | solana-labs/solana | 2020-02-12 | - | - | - |
| 108 | v0.23.5 | solana-labs/solana | 2020-02-15 | - | - | - |
| 109 | v0.23.6 | solana-labs/solana | 2020-02-22 | - | - | - |
| 110 | v0.23.7 | solana-labs/solana | 2020-02-27 | - | - | - |
| 111 | v0.23.8 | solana-labs/solana | 2020-02-29 | - | - | - |
| 112 | v1.0.0 | solana-labs/solana | 2020-02-25 | - | - | - |
| 113 | v1.0.1 | solana-labs/solana | 2020-03-01 | - | - | - |
| 114 | v1.0.2 | solana-labs/solana | 2020-03-03 | - | - | - |
| 115 | v1.0.3 | solana-labs/solana | 2020-03-05 | - | - | - |
| 116 | v1.0.4 | solana-labs/solana | 2020-03-06 | - | - | - |
| 117 | v1.0.5 | solana-labs/solana | 2020-03-10 | - | - | - |
| 118 | v1.0.6 | solana-labs/solana | 2020-03-13 | - | - | - |
| 119 | v1.0.7 | solana-labs/solana | 2020-03-16 | - | - | - |
| 120 | v1.0.8 | solana-labs/solana | 2020-03-19 | - | - | - |
| 121 | v1.0.9 | solana-labs/solana | 2020-03-24 | - | - | - |
| 122 | v1.0.10 | solana-labs/solana | 2020-03-26 | - | - | - |
| 123 | v1.0.11 | solana-labs/solana | 2020-03-30 | - | - | - |
| 124 | v1.0.12 | solana-labs/solana | 2020-03-31 | - | - | - |
| 125 | v1.0.13 | solana-labs/solana | 2020-04-03 | - | - | - |
| 126 | v1.0.14 | solana-labs/solana | 2020-04-07 | - | - | - |
| 127 | v1.0.15 | solana-labs/solana | 2020-04-09 | - | - | - |
| 128 | v1.0.16 | solana-labs/solana | 2020-04-12 | - | - | - |
| 129 | v1.0.17 | solana-labs/solana | 2020-04-14 | - | - | - |
| 130 | v1.0.18 | solana-labs/solana | 2020-04-16 | - | - | - |
| 131 | v1.0.19 | solana-labs/solana | 2020-04-23 | - | - | - |
| 132 | v1.0.20 | solana-labs/solana | 2020-04-29 | - | - | - |
| 133 | v1.0.21 | solana-labs/solana | 2020-05-02 | - | - | - |
| 134 | v1.0.22 | solana-labs/solana | 2020-05-11 | - | - | - |
| 135 | v1.0.23 | solana-labs/solana | 2020-05-21 | - | - | - |
| 136 | v1.0.24 | solana-labs/solana | 2020-05-26 | - | - | - |
| 137 | v1.1.0 | solana-labs/solana | 2020-03-31 | - | - | - |
| 138 | v1.1.1 | solana-labs/solana | 2020-04-03 | - | - | - |
| 139 | v1.1.2 | solana-labs/solana | 2020-04-07 | - | - | - |
| 140 | v1.1.3 | solana-labs/solana | 2020-04-13 | - | - | - |
| 141 | v1.1.4 | solana-labs/solana | 2020-04-16 | - | - | - |
| 142 | v1.1.5 | solana-labs/solana | 2020-04-19 | - | - | - |
| 143 | v1.1.6 | solana-labs/solana | 2020-04-23 | - | - | - |
| 144 | v1.1.7 | solana-labs/solana | 2020-04-24 | - | - | - |
| 145 | v1.1.8 | solana-labs/solana | 2020-05-02 | - | - | - |
| 146 | v1.1.9 | solana-labs/solana | 2020-05-06 | - | - | - |
| 147 | v1.1.10 | solana-labs/solana | 2020-05-07 | - | - | - |
| 148 | v1.1.11 | solana-labs/solana | 2020-05-12 | - | - | - |
| 149 | v1.1.12 | solana-labs/solana | 2020-05-12 | - | - | - |
| 150 | v1.1.13 | solana-labs/solana | 2020-05-18 | - | - | - |
| 151 | v1.1.14 | solana-labs/solana | 2020-05-27 | - | - | - |
| 152 | v1.1.15 | solana-labs/solana | 2020-05-28 | - | - | - |
| 153 | v1.1.16 | solana-labs/solana | 2020-06-08 | - | - | - |
| 154 | v1.1.17 | solana-labs/solana | 2020-06-11 | - | - | - |
| 155 | v1.1.18 | solana-labs/solana | 2020-06-15 | - | - | - |
| 156 | v1.1.19 | solana-labs/solana | 2020-06-18 | - | - | - |
| 157 | v1.1.20 | solana-labs/solana | 2020-07-22 | - | - | - |
| 158 | v1.1.21 | solana-labs/solana | 2020-07-22 | - | - | - |
| 159 | v1.1.22 | solana-labs/solana | 2020-07-27 | - | - | - |
| 160 | v1.1.23 | solana-labs/solana | 2020-07-30 | - | - | - |
| 161 | v1.2.0 | solana-labs/solana | 2020-05-27 | - | - | - |
| 162 | v1.2.1 | solana-labs/solana | 2020-06-11 | - | - | - |
| 163 | v1.2.2 | solana-labs/solana | 2020-06-14 | - | - | - |
| 164 | v1.2.3 | solana-labs/solana | 2020-06-17 | - | - | - |
| 165 | v1.2.4 | solana-labs/solana | 2020-06-18 | - | - | - |
| 166 | v1.2.5 | solana-labs/solana | 2020-06-23 | - | - | - |
| 167 | v1.2.6 | solana-labs/solana | 2020-06-23 | - | - | - |
| 168 | v1.2.7 | solana-labs/solana | 2020-06-30 | - | - | - |
| 169 | v1.2.8 | solana-labs/solana | 2020-06-30 | - | - | - |
| 170 | v1.2.9 | solana-labs/solana | 2020-07-03 | - | - | - |
| 171 | v1.2.10 | solana-labs/solana | 2020-07-08 | - | - | - |
| 172 | v1.2.11 | solana-labs/solana | 2020-07-15 | - | - | - |
| 173 | v1.2.12 | solana-labs/solana | 2020-07-22 | - | - | - |
| 174 | v1.2.13 | solana-labs/solana | 2020-07-24 | - | - | - |
| 175 | v1.2.14 | solana-labs/solana | 2020-07-29 | - | - | - |
| 176 | v1.2.15 | solana-labs/solana | 2020-07-30 | - | - | - |
| 177 | v1.2.16 | solana-labs/solana | 2020-08-01 | - | - | - |
| 178 | v1.2.17 | solana-labs/solana | 2020-08-02 | - | - | - |
| 179 | v1.2.18 | solana-labs/solana | 2020-08-05 | - | - | - |
| 180 | v1.2.19 | solana-labs/solana | 2020-08-06 | - | - | - |
| 181 | v1.2.20 | solana-labs/solana | 2020-08-06 | - | - | - |
| 182 | v1.2.21 | solana-labs/solana | 2020-08-09 | - | - | - |
| 183 | v1.2.22 | solana-labs/solana | 2020-08-11 | - | - | - |
| 184 | v1.2.23 | solana-labs/solana | 2020-08-14 | - | - | - |
| 185 | v1.2.24 | solana-labs/solana | 2020-08-18 | - | - | - |
| 186 | v1.2.25 | solana-labs/solana | 2020-08-19 | - | - | - |
| 187 | v1.2.26 | solana-labs/solana | 2020-08-20 | - | - | - |
| 188 | v1.2.27 | solana-labs/solana | 2020-09-01 | - | - | - |
| 189 | v1.2.28 | solana-labs/solana | 2020-09-05 | - | - | - |
| 190 | v1.2.29 | solana-labs/solana | 2020-09-17 | - | - | - |
| 191 | v1.2.30 | solana-labs/solana | 2020-09-28 | - | - | - |
| 192 | v1.2.31 | solana-labs/solana | 2020-09-29 | - | - | - |
| 193 | v1.2.32 | solana-labs/solana | 2020-10-02 | - | - | - |
| 194 | v1.3.0 | solana-labs/solana | 2020-08-07 | - | - | - |
| 195 | v1.3.1 | solana-labs/solana | 2020-08-08 | - | - | - |
| 196 | v1.3.2 | solana-labs/solana | 2020-08-11 | - | - | - |
| 197 | v1.3.3 | solana-labs/solana | 2020-08-14 | - | - | - |
| 198 | v1.3.4 | solana-labs/solana | 2020-08-19 | - | - | - |
| 199 | v1.3.5 | solana-labs/solana | 2020-08-28 | - | - | - |
| 200 | v1.3.6 | solana-labs/solana | 2020-08-29 | - | - | - |
| 201 | v1.3.7 | solana-labs/solana | 2020-08-31 | - | - | - |
| 202 | v1.3.8 | solana-labs/solana | 2020-09-03 | - | - | - |
| 203 | v1.3.9 | solana-labs/solana | 2020-09-09 | - | - | - |
| 204 | v1.3.10 | solana-labs/solana | 2020-09-15 | - | - | - |
| 205 | v1.3.11 | solana-labs/solana | 2020-09-15 | - | - | - |
| 206 | v1.3.12 | solana-labs/solana | 2020-09-20 | - | - | - |
| 207 | v1.3.13 | solana-labs/solana | 2020-09-24 | - | - | - |
| 208 | v1.3.14 | solana-labs/solana | 2020-09-29 | - | - | - |
| 209 | v1.3.15 | solana-labs/solana | 2020-10-03 | - | - | - |
| 210 | v1.3.16 | solana-labs/solana | 2020-10-10 | - | - | - |
| 211 | v1.3.17 | solana-labs/solana | 2020-10-13 | - | - | - |
| 212 | v1.3.18 | solana-labs/solana | 2020-10-19 | - | - | - |
| 213 | v1.3.19 | solana-labs/solana | 2020-10-21 | - | - | - |
| 214 | v1.3.20 | solana-labs/solana | 2020-11-11 | - | - | - |
| 215 | v1.3.21 | solana-labs/solana | 2020-11-13 | - | - | - |
| 216 | v1.3.22 | solana-labs/solana | 2020-11-25 | - | - | - |
| 217 | v1.3.23 | solana-labs/solana | 2020-11-29 | - | - | - |
| 218 | v1.4.0 | solana-labs/solana | 2020-10-14 | - | - | - |
| 219 | v1.4.1 | solana-labs/solana | 2020-10-15 | - | - | - |
| 220 | v1.4.2 | solana-labs/solana | 2020-10-23 | - | - | - |
| 221 | v1.4.3 | solana-labs/solana | 2020-10-24 | - | - | - |
| 222 | v1.4.4 | solana-labs/solana | 2020-11-01 | - | - | - |
| 223 | v1.4.5 | solana-labs/solana | 2020-11-07 | - | - | - |
| 224 | v1.4.6 | solana-labs/solana | 2020-11-10 | - | - | - |
| 225 | v1.4.7 | solana-labs/solana | 2020-11-12 | - | - | - |
| 226 | v1.4.8 | solana-labs/solana | 2020-11-14 | - | - | - |
| 227 | v1.4.9 | solana-labs/solana | 2020-11-18 | - | - | - |
| 228 | v1.4.11 | solana-labs/solana | 2020-11-24 | - | - | - |
| 229 | v1.4.12 | solana-labs/solana | 2020-11-26 | - | - | - |
| 230 | v1.4.13 | solana-labs/solana | 2020-11-29 | - | - | - |
| 231 | v1.4.14 | solana-labs/solana | 2020-12-03 | - | - | - |
| 232 | v1.4.16 | solana-labs/solana | 2020-12-11 | - | - | - |
| 233 | v1.4.17 | solana-labs/solana | 2020-12-16 | - | - | - |
| 234 | v1.4.18 | solana-labs/solana | 2020-12-17 | - | - | - |
| 235 | v1.4.19 | solana-labs/solana | 2020-12-21 | - | - | - |
| 236 | v1.4.20 | solana-labs/solana | 2021-01-01 | - | - | - |
| 237 | v1.4.21 | solana-labs/solana | 2021-01-06 | - | - | - |
| 238 | v1.4.22 | solana-labs/solana | 2021-01-08 | - | - | - |
| 239 | v1.4.23 | solana-labs/solana | 2021-01-12 | - | - | - |
| 240 | v1.4.24 | solana-labs/solana | 2021-01-20 | - | - | - |
| 241 | v1.4.25 | solana-labs/solana | 2021-01-29 | - | - | - |
| 242 | v1.4.26 | solana-labs/solana | 2021-02-05 | - | - | - |
| 243 | v1.4.27 | solana-labs/solana | 2021-02-13 | - | - | - |
| 244 | v1.4.28 | solana-labs/solana | 2021-02-17 | - | - | - |
| 245 | v1.5.0 | solana-labs/solana | 2020-12-17 | - | - | - |
| 246 | v1.5.1 | solana-labs/solana | 2021-01-04 | - | - | - |
| 247 | v1.5.2 | solana-labs/solana | 2021-01-08 | - | - | - |
| 248 | v1.5.3 | solana-labs/solana | 2021-01-14 | - | - | - |
| 249 | v1.5.4 | solana-labs/solana | 2021-01-20 | - | - | - |
| 250 | v1.5.5 | solana-labs/solana | 2021-01-27 | - | - | - |
| 251 | v1.5.6 | solana-labs/solana | 2021-02-10 | - | - | - |
| 252 | v1.5.7 | solana-labs/solana | 2021-02-13 | - | - | - |
| 253 | v1.5.8 | solana-labs/solana | 2021-02-17 | - | - | - |
| 254 | v1.5.9 | solana-labs/solana | - | - | - | - |
| 255 | v1.5.10 | solana-labs/solana | 2021-02-25 | - | - | - |
| 256 | v1.5.11 | solana-labs/solana | 2021-02-27 | - | - | - |
| 257 | v1.5.12 | solana-labs/solana | 2021-03-03 | - | - | - |
| 258 | v1.5.13 | solana-labs/solana | 2021-03-03 | - | - | - |
| 259 | v1.5.14 | solana-labs/solana | 2021-03-09 | - | - | - |
| 260 | v1.5.15 | solana-labs/solana | 2021-03-16 | - | - | - |
| 261 | v1.5.16 | solana-labs/solana | 2021-03-19 | - | - | - |
| 262 | v1.5.17 | solana-labs/solana | 2021-03-30 | - | - | - |
| 263 | v1.5.18 | solana-labs/solana | 2021-04-01 | - | - | - |
| 264 | v1.5.19 | solana-labs/solana | 2021-04-20 | - | - | - |
| 265 | v1.6.0 | solana-labs/solana | 2021-03-15 | - | - | - |
| 266 | v1.6.1 | solana-labs/solana | 2021-03-18 | - | - | - |
| 267 | v1.6.2 | solana-labs/solana | 2021-03-30 | - | - | - |
| 268 | v1.6.3 | solana-labs/solana | 2021-04-01 | - | - | - |
| 269 | v1.6.4 | solana-labs/solana | 2021-04-04 | - | - | - |
| 270 | v1.6.5 | solana-labs/solana | 2021-04-14 | - | - | - |
| 271 | v1.6.6 | solana-labs/solana | 2021-04-16 | - | - | - |
| 272 | v1.6.7 | solana-labs/solana | 2021-05-04 | - | - | - |
| 273 | v1.6.8 | solana-labs/solana | 2021-05-08 | - | - | - |
| 274 | v1.6.9 | solana-labs/solana | 2021-05-15 | - | - | - |
| 275 | v1.6.10 | solana-labs/solana | 2021-05-25 | - | - | - |
| 276 | v1.6.11 | solana-labs/solana | 2021-06-02 | - | - | - |
| 277 | v1.6.12 | solana-labs/solana | 2021-06-15 | - | - | - |
| 278 | v1.6.13 | solana-labs/solana | 2021-06-17 | - | - | - |
| 279 | v1.6.14 | solana-labs/solana | 2021-06-21 | - | - | - |
| 280 | v1.6.15 | solana-labs/solana | 2021-07-01 | - | - | - |
| 281 | v1.6.16 | solana-labs/solana | 2021-07-02 | - | - | - |
| 282 | v1.6.17 | solana-labs/solana | 2021-07-16 | - | - | - |
| 283 | v1.6.18 | solana-labs/solana | 2021-07-17 | - | - | - |
| 284 | v1.6.19 | solana-labs/solana | 2021-07-17 | - | - | - |
| 285 | v1.6.20 | solana-labs/solana | 2021-07-23 | - | - | - |
| 286 | v1.6.21 | solana-labs/solana | 2021-08-18 | - | - | - |
| 287 | v1.6.22 | solana-labs/solana | 2021-08-26 | - | - | - |
| 288 | v1.6.23 | solana-labs/solana | 2021-09-14 | - | - | - |
| 289 | v1.6.24 | solana-labs/solana | 2021-09-14 | - | - | - |
| 290 | v1.6.25 | solana-labs/solana | 2021-09-15 | - | - | - |
| 291 | v1.6.26 | solana-labs/solana | 2021-09-17 | - | - | - |
| 292 | v1.6.27 | solana-labs/solana | 2021-09-28 | - | - | - |
| 293 | v1.6.28 | solana-labs/solana | 2021-10-07 | - | - | - |
| 294 | v1.7.0 | solana-labs/solana | 2021-06-02 | - | - | - |
| 295 | v1.7.1 | solana-labs/solana | 2021-06-08 | - | - | - |
| 296 | v1.7.2 | solana-labs/solana | 2021-06-17 | - | - | - |
| 297 | v1.7.3 | solana-labs/solana | 2021-06-22 | - | - | - |
| 298 | v1.7.4 | solana-labs/solana | 2021-07-01 | - | - | - |
| 299 | v1.7.5 | solana-labs/solana | 2021-07-16 | - | - | - |
| 300 | v1.7.6 | solana-labs/solana | 2021-07-17 | - | - | - |
| 301 | v1.7.7 | solana-labs/solana | 2021-07-23 | - | - | - |
| 302 | v1.7.8 | solana-labs/solana | 2021-07-23 | - | - | - |
| 303 | v1.7.9 | solana-labs/solana | 2021-07-31 | - | - | - |
| 304 | v1.7.10 | solana-labs/solana | 2021-08-12 | - | - | - |
| 305 | v1.7.11 | solana-labs/solana | 2021-08-27 | - | - | - |
| 306 | v1.7.12 | solana-labs/solana | 2021-09-16 | - | - | - |
| 307 | v1.7.13 | solana-labs/solana | 2021-09-29 | - | - | - |
| 308 | v1.7.14 | solana-labs/solana | 2021-09-30 | - | - | - |
| 309 | v1.7.15 | solana-labs/solana | 2021-10-07 | - | - | - |
| 310 | v1.7.16 | solana-labs/solana | 2021-10-22 | - | - | - |
| 311 | v1.7.17 | solana-labs/solana | 2021-10-27 | - | - | - |
| 312 | v1.8.0 | solana-labs/solana | 2021-10-06 | - | - | - |
| 313 | v1.8.1 | solana-labs/solana | 2021-10-21 | - | - | - |
| 314 | v1.8.2 | solana-labs/solana | 2021-10-27 | - | - | - |
| 315 | v1.8.3 | solana-labs/solana | 2021-11-10 | - | - | - |
| 316 | v1.8.4 | solana-labs/solana | 2021-11-16 | - | - | - |
| 317 | v1.8.5 | solana-labs/solana | 2021-11-17 | - | - | - |
| 318 | v1.8.6 | solana-labs/solana | 2021-12-01 | - | - | - |
| 319 | v1.8.7 | solana-labs/solana | 2021-12-04 | - | - | - |
| 320 | v1.8.8 | solana-labs/solana | 2021-12-08 | - | - | - |
| 321 | v1.8.9 | solana-labs/solana | 2021-12-10 | - | - | - |
| 322 | v1.8.10 | solana-labs/solana | 2021-12-13 | - | - | - |
| 323 | v1.8.11 | solana-labs/solana | 2021-12-15 | - | - | - |
| 324 | v1.8.12 | solana-labs/solana | 2022-01-08 | - | - | - |
| 325 | v1.8.13 | solana-labs/solana | 2022-01-19 | - | - | - |
| 326 | v1.8.14 | solana-labs/solana | 2022-01-22 | - | - | - |
| 327 | v1.8.15 | solana-labs/solana | 2022-02-18 | - | - | - |
| 328 | v1.8.16 | solana-labs/solana | 2022-02-18 | - | - | - |
| 329 | v1.9.0 | solana-labs/solana | 2021-12-11 | - | - | - |
| 330 | v1.9.1 | solana-labs/solana | 2021-12-16 | - | - | - |
| 331 | v1.9.2 | solana-labs/solana | 2021-12-22 | - | - | - |
| 332 | v1.9.3 | solana-labs/solana | 2022-01-05 | - | - | - |
| 333 | v1.9.4 | solana-labs/solana | 2022-01-08 | - | - | - |
| 334 | v1.9.5 | solana-labs/solana | 2022-01-22 | - | - | - |
| 335 | v1.9.6 | solana-labs/solana | 2022-02-11 | - | - | - |
| 336 | v1.9.7 | solana-labs/solana | 2022-02-17 | - | - | - |
| 337 | v1.9.8 | solana-labs/solana | 2022-02-20 | - | - | - |
| 338 | v1.9.9 | solana-labs/solana | 2022-02-23 | - | - | - |
| 339 | v1.9.10 | solana-labs/solana | 2022-03-08 | - | - | - |
| 340 | v1.9.11 | solana-labs/solana | 2022-03-10 | - | - | - |
| 341 | v1.9.12 | solana-labs/solana | 2022-03-11 | - | - | - |
| 342 | v1.9.13 | solana-labs/solana | 2022-03-16 | - | - | - |
| 343 | v1.9.14 | solana-labs/solana | 2022-03-26 | - | - | - |
| 344 | v1.9.15 | solana-labs/solana | 2022-04-09 | - | - | - |
| 345 | v1.9.16 | solana-labs/solana | 2022-04-14 | - | - | - |
| 346 | v1.9.17 | solana-labs/solana | 2022-04-19 | - | - | - |
| 347 | v1.9.18 | solana-labs/solana | 2022-04-22 | - | - | - |
| 348 | v1.9.19 | solana-labs/solana | 2022-05-04 | - | - | - |
| 349 | v1.9.20 | solana-labs/solana | 2022-05-05 | - | - | - |
| 350 | v1.9.21 | solana-labs/solana | 2022-05-09 | - | - | - |
| 351 | v1.9.22 | solana-labs/solana | 2022-05-16 | - | - | - |
| 352 | v1.9.23 | solana-labs/solana | - | - | - | - |
| 353 | v1.9.24 | solana-labs/solana | 2022-05-24 | - | - | - |
| 354 | v1.9.25 | solana-labs/solana | 2022-05-27 | - | - | - |
| 355 | v1.9.26 | solana-labs/solana | 2022-06-01 | - | - | - |
| 356 | v1.9.27 | solana-labs/solana | 2022-06-01 | - | - | - |
| 357 | v1.9.28 | solana-labs/solana | 2022-06-01 | - | - | - |
| 358 | v1.9.29 | solana-labs/solana | 2022-06-12 | - | - | - |
| 359 | v1.10.0 | solana-labs/solana | 2022-03-02 | - | - | - |
| 360 | v1.10.1 | solana-labs/solana | 2022-03-10 | - | - | - |
| 361 | v1.10.2 | solana-labs/solana | 2022-03-14 | - | - | - |
| 362 | v1.10.3 | solana-labs/solana | 2022-03-17 | - | - | - |
| 363 | v1.10.4 | solana-labs/solana | 2022-03-26 | - | - | - |
| 364 | v1.10.5 | solana-labs/solana | 2022-03-28 | - | - | - |
| 365 | v1.10.6 | solana-labs/solana | 2022-04-04 | - | - | - |
| 366 | v1.10.7 | solana-labs/solana | 2022-04-09 | - | - | - |
| 367 | v1.10.8 | solana-labs/solana | 2022-04-11 | - | - | - |
| 368 | v1.10.9 | solana-labs/solana | 2022-04-19 | - | - | - |
| 369 | v1.10.10 | solana-labs/solana | 2022-04-26 | - | - | - |
| 370 | v1.10.11 | solana-labs/solana | 2022-04-28 | - | - | - |
| 371 | v1.10.12 | solana-labs/solana | 2022-05-04 | - | - | - |
| 372 | v1.10.13 | solana-labs/solana | 2022-05-09 | - | - | - |
| 373 | v1.10.14 | solana-labs/solana | 2022-05-16 | - | - | - |
| 374 | v1.10.15 | solana-labs/solana | 2022-05-17 | - | - | - |
| 375 | v1.10.16 | solana-labs/solana | - | - | - | - |
| 376 | v1.10.17 | solana-labs/solana | 2022-05-18 | - | - | - |
| 377 | v1.10.18 | solana-labs/solana | - | - | - | - |
| 378 | v1.10.19 | solana-labs/solana | 2022-05-24 | - | - | - |
| 379 | v1.10.20 | solana-labs/solana | 2022-05-27 | - | - | - |
| 380 | v1.10.21 | solana-labs/solana | 2022-06-01 | - | - | - |
| 381 | v1.10.22 | solana-labs/solana | 2022-06-01 | - | - | - |
| 382 | v1.10.23 | solana-labs/solana | 2022-06-01 | - | - | - |
| 383 | v1.10.24 | solana-labs/solana | 2022-06-06 | - | - | - |
| 384 | v1.10.25 | solana-labs/solana | 2022-06-12 | - | - | - |
| 385 | v1.10.26 | solana-labs/solana | 2022-06-18 | - | - | - |
| 386 | v1.10.27 | solana-labs/solana | - | - | - | - |
| 387 | v1.10.28 | solana-labs/solana | 2022-06-28 | - | - | - |
| 388 | v1.10.29 | solana-labs/solana | 2022-06-29 | - | - | - |
| 389 | v1.10.30 | solana-labs/solana | 2022-07-07 | - | - | - |
| 390 | v1.10.31 | solana-labs/solana | 2022-07-12 | - | - | - |
| 391 | v1.10.32 | solana-labs/solana | 2022-07-23 | - | - | - |
| 392 | v1.10.33 | solana-labs/solana | 2022-07-26 | - | - | - |
| 393 | v1.10.34 | solana-labs/solana | 2022-08-03 | - | - | - |
| 394 | v1.10.35 | solana-labs/solana | 2022-08-16 | - | - | - |
| 395 | v1.10.36 | solana-labs/solana | - | - | - | - |
| 396 | v1.10.37 | solana-labs/solana | - | - | - | - |
| 397 | v1.10.38 | solana-labs/solana | 2022-08-26 | - | - | - |
| 398 | v1.10.39 | solana-labs/solana | 2022-09-16 | - | - | - |
| 399 | v1.10.40 | solana-labs/solana | 2022-10-03 | - | - | - |
| 400 | v1.10.41 | solana-labs/solana | 2022-10-11 | - | - | - |
| 401 | v1.11.0 | solana-labs/solana | 2022-06-21 | - | - | - |
| 402 | v1.11.1 | solana-labs/solana | 2022-06-22 | - | - | - |
| 403 | v1.11.2 | solana-labs/solana | 2022-07-07 | - | - | - |
| 404 | v1.11.3 | solana-labs/solana | 2022-07-12 | - | - | - |
| 405 | v1.11.4 | solana-labs/solana | 2022-07-23 | - | - | - |
| 406 | v1.11.5 | solana-labs/solana | 2022-08-03 | - | - | - |
| 407 | v1.11.6 | solana-labs/solana | 2022-08-17 | - | - | - |
| 408 | v1.11.7 | solana-labs/solana | - | - | - | - |
| 409 | v1.11.8 | solana-labs/solana | - | - | - | - |
| 410 | v1.11.9 | solana-labs/solana | - | - | - | - |
| 411 | v1.11.10 | solana-labs/solana | 2022-08-26 | - | - | - |
| 412 | v1.13.0 | solana-labs/solana | - | - | - | - |
| 413 | v1.13.1 | solana-labs/solana | 2022-09-20 | - | - | - |
| 414 | v1.13.2 | solana-labs/solana | 2022-10-03 | - | - | - |
| 415 | v1.13.3 | solana-labs/solana | 2022-10-11 | - | - | - |
| 416 | v1.13.4 | solana-labs/solana | 2022-10-22 | - | - | - |
| 417 | v1.13.5 | solana-labs/solana | 2022-11-11 | - | - | - |
| 418 | v1.13.6 | solana-labs/solana | 2023-01-11 | - | - | - |
| 419 | v1.13.7 | solana-labs/solana | 2023-03-23 | - | - | - |
| 420 | v1.14.0 | solana-labs/solana | - | - | - | - |
| 421 | v1.14.1 | solana-labs/solana | 2022-09-14 | - | - | - |
| 422 | v1.14.2 | solana-labs/solana | 2022-09-16 | - | - | - |
| 423 | v1.14.3 | solana-labs/solana | 2022-09-26 | - | - | - |
| 424 | v1.14.4 | solana-labs/solana | 2022-10-03 | - | - | - |
| 425 | v1.14.5 | solana-labs/solana | 2022-10-04 | - | - | - |
| 426 | v1.14.6 | solana-labs/solana | 2022-10-25 | - | - | - |
| 427 | v1.14.7 | solana-labs/solana | 2022-11-01 | - | - | - |
| 428 | v1.14.8 | solana-labs/solana | 2022-11-22 | - | - | - |
| 429 | v1.14.9 | solana-labs/solana | 2022-11-28 | - | - | - |
| 430 | v1.14.10 | solana-labs/solana | 2022-12-05 | - | - | - |
| 431 | v1.14.11 | solana-labs/solana | 2022-12-20 | - | - | - |
| 432 | v1.14.12 | solana-labs/solana | 2023-01-10 | - | - | - |
| 433 | v1.14.13 | solana-labs/solana | 2023-01-19 | - | - | - |
| 434 | v1.14.14 | solana-labs/solana | 2023-02-06 | - | - | - |
| 435 | v1.14.15 | solana-labs/solana | 2023-02-08 | - | - | - |
| 436 | v1.14.16 | solana-labs/solana | 2023-02-17 | - | - | - |
| 437 | v1.14.17 | solana-labs/solana | 2023-03-23 | - | - | - |
| 438 | v1.14.18 | solana-labs/solana | 2023-05-16 | - | - | - |
| 439 | v1.14.19 | solana-labs/solana | 2023-06-15 | - | - | - |
| 440 | v1.14.20 | solana-labs/solana | 2023-07-08 | - | - | - |
| 441 | v1.14.21 | solana-labs/solana | 2023-07-22 | - | - | - |
| 442 | v1.14.22 | solana-labs/solana | 2023-07-28 | - | - | - |
| 443 | v1.14.23 | solana-labs/solana | 2023-08-05 | - | - | - |
| 444 | v1.14.24 | solana-labs/solana | 2023-08-11 | - | - | - |
| 445 | v1.14.25 | solana-labs/solana | 2023-08-25 | - | - | - |
| 446 | v1.14.26 | solana-labs/solana | 2023-09-01 | - | - | - |
| 447 | v1.14.27 | solana-labs/solana | 2023-09-08 | - | - | - |
| 448 | v1.14.28 | solana-labs/solana | 2023-09-21 | - | - | - |
| 449 | v1.14.29 | solana-labs/solana | 2023-09-29 | - | - | - |
| 450 | v1.15.0 | solana-labs/solana | 2023-02-02 | - | - | - |
| 451 | v1.15.1 | solana-labs/solana | 2023-02-09 | - | - | - |
| 452 | v1.15.2 | solana-labs/solana | 2023-02-14 | - | - | - |
| 453 | v1.16.0 | solana-labs/solana | 2023-05-31 | - | - | - |
| 454 | v1.16.1 | solana-labs/solana | 2023-06-15 | - | - | - |
| 455 | v1.16.2 | solana-labs/solana | 2023-06-30 | - | - | - |
| 456 | v1.16.3 | solana-labs/solana | 2023-07-07 | - | - | - |
| 457 | v1.16.4 | solana-labs/solana | 2023-07-14 | - | - | - |
| 458 | v1.16.5 | solana-labs/solana | 2023-07-22 | - | - | - |
| 459 | v1.16.6 | solana-labs/solana | 2023-07-28 | - | - | - |
| 460 | v1.16.7 | solana-labs/solana | 2023-08-05 | - | - | - |
| 461 | v1.16.8 | solana-labs/solana | 2023-08-11 | - | - | - |
| 462 | v1.16.9 | solana-labs/solana | 2023-08-18 | - | - | - |
| 463 | v1.16.10 | solana-labs/solana | 2023-08-25 | - | - | - |
| 464 | v1.16.11 | solana-labs/solana | 2023-08-31 | - | - | - |
| 465 | v1.16.12 | solana-labs/solana | 2023-09-01 | - | - | - |
| 466 | v1.16.13 | solana-labs/solana | 2023-09-08 | - | - | - |
| 467 | v1.16.14 | solana-labs/solana | 2023-09-21 | - | - | - |
| 468 | v1.16.15 | solana-labs/solana | 2023-09-29 | - | - | - |
| 469 | v1.16.16 | solana-labs/solana | 2023-10-06 | - | - | - |
| 470 | v1.16.17 | solana-labs/solana | 2023-10-13 | - | - | - |
| 471 | v1.16.18 | solana-labs/solana | 2023-10-20 | - | - | - |
| 472 | v1.16.19 | solana-labs/solana | 2023-11-10 | - | - | - |
| 473 | v1.16.20 | solana-labs/solana | 2023-11-17 | - | - | - |
| 474 | v1.16.21 | solana-labs/solana | 2023-12-03 | - | - | - |
| 475 | v1.16.22 | solana-labs/solana | 2023-12-09 | - | - | - |
| 476 | v1.16.23 | solana-labs/solana | 2023-12-10 | - | - | - |
| 477 | v1.16.24 | solana-labs/solana | 2023-12-15 | - | - | - |
| 478 | v1.16.25 | solana-labs/solana | 2024-01-03 | - | - | - |
| 479 | v1.16.26 | solana-labs/solana | 2024-01-07 | - | - | - |
| 480 | v1.16.27 | solana-labs/solana | 2024-01-10 | - | - | - |
| 481 | v1.17.0 | solana-labs/solana | 2023-10-02 | - | - | - |
| 482 | v1.17.1 | solana-labs/solana | 2023-10-06 | - | - | - |
| 483 | v1.17.2 | solana-labs/solana | 2023-10-16 | - | - | - |
| 484 | v1.17.3 | solana-labs/solana | 2023-10-20 | - | - | - |
| 485 | v1.17.4 | solana-labs/solana | 2023-10-30 | - | - | - |
| 486 | v1.17.5 | solana-labs/solana | 2023-11-10 | - | - | - |
| 487 | v1.17.6 | solana-labs/solana | 2023-11-17 | - | - | - |
| 488 | v1.17.7 | solana-labs/solana | 2023-11-30 | - | - | - |
| 489 | v1.17.8 | solana-labs/solana | 2023-12-09 | - | - | - |
| 490 | v1.17.9 | solana-labs/solana | 2023-12-10 | - | - | - |
| 491 | v1.17.10 | solana-labs/solana | 2023-12-13 | - | - | - |
| 492 | v1.17.11 | solana-labs/solana | 2023-12-15 | - | - | - |
| 493 | v1.17.12 | solana-labs/solana | 2023-12-17 | - | - | - |
| 494 | v1.17.13 | solana-labs/solana | 2023-12-21 | - | - | - |
| 495 | v1.17.14 | solana-labs/solana | 2024-01-02 | - | - | - |
| 496 | v1.17.15 | solana-labs/solana | 2024-01-07 | - | - | - |
| 497 | v1.17.16 | solana-labs/solana | 2024-01-12 | - | - | - |
| 498 | v1.17.17 | solana-labs/solana | 2024-01-20 | - | - | - |
| 499 | v1.17.18 | solana-labs/solana | 2024-01-26 | - | - | - |
| 500 | v1.17.19 | solana-labs/solana | 2024-02-06 | - | - | - |
| 501 | v1.17.20 | solana-labs/solana | 2024-02-06 | - | - | - |
| 502 | v1.17.21 | solana-labs/solana | 2024-02-13 | - | - | - |
| 503 | v1.17.22 | solana-labs/solana | 2024-02-13 | - | - | - |
| 504 | v1.17.23 | solana-labs/solana | 2024-02-24 | - | - | - |
| 505 | v1.17.24 | solana-labs/solana | 2024-03-03 | - | - | - |
| 506 | v1.17.25 | anza-xyz/agave | 2024-03-09 | - | - | - |
| 507 | v1.17.25 | solana-labs/solana | 2024-03-09 | - | - | - |
| 508 | v1.17.26 | anza-xyz/agave | 2024-03-13 | - | - | - |
| 509 | v1.17.26 | solana-labs/solana | 2024-03-13 | - | - | - |
| 510 | v1.17.27 | anza-xyz/agave | 2024-03-18 | - | - | - |
| 511 | v1.17.27 | solana-labs/solana | 2024-03-18 | - | - | - |
| 512 | v1.17.28 | anza-xyz/agave | 2024-03-23 | - | - | - |
| 513 | v1.17.28 | solana-labs/solana | 2024-03-23 | - | - | - |
| 514 | v1.17.29 | anza-xyz/agave | 2024-03-29 | - | - | - |
| 515 | v1.17.29 | solana-labs/solana | 2024-03-29 | - | - | - |
| 516 | v1.17.30 | anza-xyz/agave | 2024-04-08 | - | - | - |
| 517 | v1.17.30 | solana-labs/solana | 2024-04-08 | - | - | - |
| 518 | v1.17.31 | anza-xyz/agave | 2024-04-12 | - | - | - |
| 519 | v1.17.31 | solana-labs/solana | 2024-04-12 | - | - | - |
| 520 | v1.17.32 | anza-xyz/agave | 2024-04-27 | - | - | - |
| 521 | v1.17.32 | solana-labs/solana | 2024-04-27 | - | - | - |
| 522 | v1.17.33 | anza-xyz/agave | 2024-05-07 | - | - | - |
| 523 | v1.17.33 | solana-labs/solana | 2024-05-07 | - | - | - |
| 524 | v1.17.34 | anza-xyz/agave | 2024-05-18 | - | - | - |
| 525 | v1.17.34 | solana-labs/solana | 2024-05-18 | - | - | - |
| 526 | v1.18.0 | solana-labs/solana | 2024-01-26 | - | - | - |
| 527 | v1.18.1 | solana-labs/solana | 2024-01-31 | - | - | - |
| 528 | v1.18.2 | solana-labs/solana | 2024-02-13 | - | - | - |
| 529 | v1.18.3 | solana-labs/solana | 2024-02-24 | - | - | - |
| 530 | v1.18.4 | solana-labs/solana | 2024-03-03 | - | - | - |
| 531 | v1.18.5 | anza-xyz/agave | 2024-03-09 | - | - | - |
| 532 | v1.18.5 | solana-labs/solana | 2024-03-09 | - | - | - |
| 533 | v1.18.6 | anza-xyz/agave | 2024-03-14 | - | - | - |
| 534 | v1.18.6 | solana-labs/solana | 2024-03-14 | - | - | - |
| 535 | v1.18.7 | anza-xyz/agave | 2024-03-18 | - | - | - |
| 536 | v1.18.7 | solana-labs/solana | 2024-03-18 | - | - | - |
| 537 | v1.18.8 | anza-xyz/agave | 2024-03-23 | - | - | - |
| 538 | v1.18.8 | solana-labs/solana | 2024-03-23 | - | - | - |
| 539 | v1.18.9 | anza-xyz/agave | 2024-03-29 | - | - | - |
| 540 | v1.18.9 | solana-labs/solana | 2024-03-29 | - | - | - |
| 541 | v1.18.10 | anza-xyz/agave | 2024-04-08 | - | - | - |
| 542 | v1.18.10 | solana-labs/solana | 2024-04-08 | - | - | - |
| 543 | v1.18.11 | anza-xyz/agave | 2024-04-11 | - | - | - |
| 544 | v1.18.11 | solana-labs/solana | 2024-04-11 | - | - | - |
| 545 | v1.18.12 | anza-xyz/agave | 2024-04-26 | - | - | - |
| 546 | v1.18.12 | solana-labs/solana | 2024-04-26 | - | - | - |
| 547 | v1.18.13 | anza-xyz/agave | 2024-05-14 | - | - | - |
| 548 | v1.18.13 | solana-labs/solana | 2024-05-14 | - | - | - |
| 549 | v1.18.14 | anza-xyz/agave | 2024-05-18 | - | - | - |
| 550 | v1.18.14 | solana-labs/solana | 2024-05-18 | - | - | - |
| 551 | v1.18.15 | anza-xyz/agave | 2024-05-31 | - | - | - |
| 552 | v1.18.15 | solana-labs/solana | 2024-05-31 | - | - | - |
| 553 | v1.18.16 | anza-xyz/agave | 2024-06-07 | - | - | - |
| 554 | v1.18.16 | solana-labs/solana | 2024-06-08 | - | - | - |
| 555 | v1.18.17 | anza-xyz/agave | 2024-06-17 | - | - | - |
| 556 | v1.18.17 | solana-labs/solana | 2024-06-17 | - | - | - |
| 557 | v1.18.18 | anza-xyz/agave | 2024-07-10 | - | - | - |
| 558 | v1.18.18 | solana-labs/solana | 2024-07-11 | - | - | - |
| 559 | v1.18.19 | anza-xyz/agave | 2024-07-20 | - | - | - |
| 560 | v1.18.19 | solana-labs/solana | 2024-07-22 | - | - | - |
| 561 | v1.18.20 | anza-xyz/agave | 2024-07-24 | - | - | - |
| 562 | v1.18.20 | solana-labs/solana | 2024-07-25 | - | - | - |
| 563 | v1.18.21 | anza-xyz/agave | 2024-08-02 | - | - | - |
| 564 | v1.18.21 | solana-labs/solana | 2024-08-03 | - | - | - |
| 565 | v1.18.22 | anza-xyz/agave | 2024-08-08 | - | - | - |
| 566 | v1.18.22 | solana-labs/solana | 2024-08-09 | - | - | - |
| 567 | v1.18.23 | anza-xyz/agave | 2024-08-30 | - | - | - |
| 568 | v1.18.23 | solana-labs/solana | 2024-08-31 | - | - | - |
| 569 | v1.18.24 | anza-xyz/agave | 2024-09-27 | - | - | - |
| 570 | v1.18.25 | anza-xyz/agave | 2024-10-04 | - | - | - |
| 571 | v1.18.25 | solana-labs/solana | 2024-10-04 | - | - | - |
| 572 | v1.18.26 | anza-xyz/agave | 2024-10-11 | - | - | - |
| 573 | v1.18.26 | solana-labs/solana | 2024-10-12 | - | - | - |
| 574 | v2.0.0 | anza-xyz/agave | 2024-06-21 | - | - | - |
| 575 | v2.0.1 | anza-xyz/agave | 2024-06-24 | - | - | - |
| 576 | v2.0.2 | anza-xyz/agave | 2024-07-10 | - | - | - |
| 577 | v2.0.3 | anza-xyz/agave | 2024-07-20 | - | - | - |
| 578 | v2.0.4 | anza-xyz/agave | 2024-08-02 | - | - | - |
| 579 | v2.0.5 | anza-xyz/agave | 2024-08-09 | - | - | - |
| 580 | v2.0.6 | anza-xyz/agave | 2024-08-18 | - | - | - |
| 581 | v2.0.7 | anza-xyz/agave | 2024-08-23 | - | - | - |
| 582 | v2.0.8 | anza-xyz/agave | 2024-08-30 | - | - | - |
| 583 | v2.0.9 | anza-xyz/agave | 2024-09-06 | - | - | - |
| 584 | v2.0.10 | anza-xyz/agave | 2024-09-16 | - | - | - |
| 585 | v2.0.11 | anza-xyz/agave | 2024-09-27 | - | - | - |
| 586 | v2.0.12 | anza-xyz/agave | 2024-10-01 | - | - | - |
| 587 | v2.0.13 | anza-xyz/agave | 2024-10-02 | - | - | - |
| 588 | v2.0.14 | anza-xyz/agave | 2024-10-17 | - | - | - |
| 589 | v2.0.15 | anza-xyz/agave | 2024-11-07 | - | - | - |
| 590 | v2.0.16 | anza-xyz/agave | 2024-11-16 | - | - | - |
| 591 | v2.0.17 | anza-xyz/agave | 2024-11-22 | - | - | - |
| 592 | v2.0.18 | anza-xyz/agave | 2024-11-25 | - | - | - |
| 593 | v2.0.19 | anza-xyz/agave | 2024-12-07 | - | - | - |
| 594 | v2.0.20 | anza-xyz/agave | 2024-12-15 | - | - | - |
| 595 | v2.0.21 | anza-xyz/agave | 2024-12-22 | - | - | - |
| 596 | v2.0.22 | anza-xyz/agave | 2025-01-12 | - | - | - |
| 597 | v2.0.23 | anza-xyz/agave | 2025-01-17 | - | - | - |
| 598 | v2.0.24 | anza-xyz/agave | 2025-01-23 | - | - | - |
| 599 | v2.0.25 | anza-xyz/agave | 2025-02-01 | - | - | - |
| 600 | v2.1.0 | anza-xyz/agave | 2024-10-30 | - | - | - |
| 601 | v2.1.1 | anza-xyz/agave | 2024-11-17 | - | - | - |
| 602 | v2.1.2 | anza-xyz/agave | 2024-11-23 | - | - | - |
| 603 | v2.1.3 | anza-xyz/agave | - | - | - | - |
| 604 | v2.1.4 | anza-xyz/agave | 2024-11-27 | - | - | - |
| 605 | v2.1.5 | anza-xyz/agave | 2024-12-07 | - | - | - |
| 606 | v2.1.6 | anza-xyz/agave | 2024-12-16 | - | - | - |
| 607 | v2.1.7 | anza-xyz/agave | 2024-12-29 | - | - | - |
| 608 | v2.1.8 | anza-xyz/agave | 2025-01-13 | - | - | - |
| 609 | v2.1.9 | anza-xyz/agave | 2025-01-15 | - | - | - |
| 610 | v2.1.10 | anza-xyz/agave | 2025-01-22 | - | - | - |
| 611 | v2.1.11 | anza-xyz/agave | 2025-01-27 | - | - | - |
| 612 | v2.1.12 | anza-xyz/agave | 2025-02-03 | - | - | - |
| 613 | v2.1.13 | anza-xyz/agave | 2025-02-04 | - | - | - |
| 614 | v2.1.14 | anza-xyz/agave | 2025-02-18 | - | - | - |
| 615 | v2.1.15 | anza-xyz/agave | 2025-03-03 | - | - | - |
| 616 | v2.1.16 | anza-xyz/agave | 2025-03-15 | - | - | - |
| 617 | v2.1.17 | anza-xyz/agave | 2025-04-01 | - | - | - |
| 618 | v2.1.18 | anza-xyz/agave | 2025-04-04 | - | - | - |
| 619 | v2.1.19 | anza-xyz/agave | 2025-04-11 | - | - | - |
| 620 | v2.1.20 | anza-xyz/agave | 2025-04-14 | - | - | - |
| 621 | v2.1.21 | anza-xyz/agave | 2025-04-18 | - | - | - |
| 622 | v2.2.0 | anza-xyz/agave | 2025-02-18 | - | - | - |
| 623 | v2.2.1 | anza-xyz/agave | 2025-03-03 | - | - | - |
| 624 | v2.2.2 | anza-xyz/agave | 2025-03-12 | - | - | - |
| 625 | v2.2.3 | anza-xyz/agave | 2025-03-15 | - | - | - |
| 626 | v2.2.4 | anza-xyz/agave | 2025-03-24 | - | - | - |
| 627 | v2.2.5 | anza-xyz/agave | - | - | - | - |
| 628 | v2.2.6 | anza-xyz/agave | 2025-03-30 | - | - | - |
| 629 | v2.2.7 | anza-xyz/agave | 2025-04-07 | - | - | - |
| 630 | v2.2.8 | anza-xyz/agave | 2025-04-11 | - | - | - |
| 631 | v2.2.9 | anza-xyz/agave | 2025-04-14 | - | - | - |
| 632 | v2.2.10 | anza-xyz/agave | 2025-04-15 | - | - | - |
| 633 | v2.2.11 | anza-xyz/agave | 2025-04-18 | - | - | - |
| 634 | v2.2.12 | anza-xyz/agave | 2025-04-23 | - | - | - |
| 635 | v2.2.13 | anza-xyz/agave | 2025-05-05 | - | - | - |
| 636 | v2.2.14 | anza-xyz/agave | 2025-05-09 | - | - | - |
| 637 | v2.2.15 | anza-xyz/agave | 2025-05-30 | - | - | - |
| 638 | v2.2.16 | anza-xyz/agave | 2025-06-13 | - | - | - |
| 639 | v2.2.17 | anza-xyz/agave | 2025-06-20 | - | - | - |
| 640 | v2.2.18 | anza-xyz/agave | 2025-06-26 | - | - | - |
| 641 | v2.2.19 | anza-xyz/agave | 2025-07-03 | - | - | - |
| 642 | v2.2.20 | anza-xyz/agave | 2025-07-11 | - | `v2-2` | 353,951,234–363,311,999 |
| 643 | v2.3.0 | anza-xyz/agave | 2025-06-13 | - | - | - |
| 644 | v2.3.1 | anza-xyz/agave | 2025-06-20 | - | - | - |
| 645 | v2.3.2 | anza-xyz/agave | 2025-06-27 | - | - | - |
| 646 | v2.3.3 | anza-xyz/agave | 2025-07-03 | - | - | - |
| 647 | v2.3.4 | anza-xyz/agave | 2025-07-11 | - | - | - |
| 648 | v2.3.5 | anza-xyz/agave | 2025-07-18 | - | - | - |
| 649 | v2.3.6 | anza-xyz/agave | 2025-07-26 | - | - | - |
| 650 | v2.3.7 | anza-xyz/agave | 2025-08-08 | - | - | - |
| 651 | v2.3.8 | anza-xyz/agave | 2025-09-05 | - | - | - |
| 652 | v2.3.9 | anza-xyz/agave | 2025-09-12 | - | `v2-3` | 363,312,000–374,504,881 |
| 653 | v2.3.10 | anza-xyz/agave | 2025-09-19 | - | - | - |
| 654 | v2.3.11 | anza-xyz/agave | 2025-09-26 | - | - | - |
| 655 | v2.3.12 | anza-xyz/agave | 2025-10-01 | - | - | - |
| 656 | v2.3.13 | anza-xyz/agave | 2025-10-10 | - | - | - |
| 657 | v3.0.0 | anza-xyz/agave | 2025-08-21 | - | - | - |
| 658 | v3.0.1 | anza-xyz/agave | 2025-09-05 | - | - | - |
| 659 | v3.0.2 | anza-xyz/agave | 2025-09-12 | - | - | - |
| 660 | v3.0.3 | anza-xyz/agave | 2025-09-19 | - | - | - |
| 661 | v3.0.4 | anza-xyz/agave | 2025-09-26 | - | - | - |
| 662 | v3.0.5 | anza-xyz/agave | 2025-10-01 | - | - | - |
| 663 | v3.0.6 | anza-xyz/agave | 2025-10-08 | - | - | - |
| 664 | v3.0.7 | anza-xyz/agave | 2025-10-17 | - | - | - |
| 665 | v3.0.8 | anza-xyz/agave | 2025-10-24 | - | - | - |
| 666 | v3.0.9 | anza-xyz/agave | 2025-11-01 | - | - | - |
| 667 | v3.0.10 | anza-xyz/agave | 2025-11-04 | - | `v3-0` | 374,504,882–398,988,489 |
| 668 | v3.0.11 | anza-xyz/agave | 2025-11-22 | - | - | - |
| 669 | v3.0.12 | anza-xyz/agave | 2025-11-29 | - | - | - |
| 670 | v3.0.13 | anza-xyz/agave | 2025-12-21 | - | - | - |
| 671 | v3.0.14 | anza-xyz/agave | 2026-01-10 | - | - | - |
| 672 | v3.1.0-beta.0 | anza-xyz/agave | 2025-11-08 | - | - | - |
| 673 | v3.1.1 | anza-xyz/agave | 2025-11-14 | - | - | - |
| 674 | v3.1.2 | anza-xyz/agave | 2025-11-22 | - | - | - |
| 675 | v3.1.3 | anza-xyz/agave | 2025-11-29 | - | - | - |
| 676 | v3.1.4 | anza-xyz/agave | 2025-12-03 | - | - | - |
| 677 | v3.1.5 | anza-xyz/agave | 2025-12-21 | - | - | - |
| 678 | v3.1.6 | anza-xyz/agave | 2026-01-10 | - | - | - |
| 679 | v3.1.7 | anza-xyz/agave | 2026-01-19 | - | - | - |
| 680 | v3.1.8 | anza-xyz/agave | 2026-01-26 | - | - | - |
| 681 | v3.1.9 | anza-xyz/agave | 2026-02-19 | - | - | - |
| 682 | v3.1.10 | anza-xyz/agave | 2026-03-10 | - | - | - |
| 683 | v3.1.11 | anza-xyz/agave | 2026-03-20 | - | `v3-1` | 398,988,490–420,440,565 |
| 684 | v3.1.12 | anza-xyz/agave | 2026-04-03 | - | - | - |
| 685 | v3.1.13 | anza-xyz/agave | 2026-04-10 | - | - | - |
| 686 | v3.1.14 | anza-xyz/agave | 2026-04-24 | - | - | - |
| 687 | v4.0.0-alpha.0 | anza-xyz/agave | 2026-02-27 | - | - | - |
| 688 | v4.0.0-beta.0 | anza-xyz/agave | 2026-03-06 | - | - | - |
| 689 | v4.0.0-beta.1 | anza-xyz/agave | 2026-03-10 | - | - | - |
| 690 | v4.0.0-beta.2 | anza-xyz/agave | 2026-03-13 | - | - | - |
| 691 | v4.0.0-beta.3 | anza-xyz/agave | 2026-03-20 | - | - | - |
| 692 | v4.0.0-beta.4 | anza-xyz/agave | 2026-03-23 | - | - | - |
| 693 | v4.0.0-beta.5 | anza-xyz/agave | 2026-03-27 | - | - | - |
| 694 | v4.0.0-beta.6 | anza-xyz/agave | 2026-04-03 | - | - | - |
| 695 | v4.0.0-beta.7 | anza-xyz/agave | 2026-04-10 | - | - | - |
| 696 | v4.0.0-rc.0 | anza-xyz/agave | 2026-04-24 | - | - | - |
| 697 | v4.0.0-rc.1 | anza-xyz/agave | 2026-05-08 | - | - | - |
| 698 | v4.0.0 | anza-xyz/agave | 2026-05-16 | - | `v4-0` | 420,440,566–429,554,363 |
| 699 | v4.0.1 | anza-xyz/agave | 2026-05-29 | - | - | - |
| 700 | v4.0.2 | anza-xyz/agave | 2026-06-05 | - | - | - |
| 701 | v4.0.3 | anza-xyz/agave | 2026-06-16 | - | - | - |
| 702 | v4.1.0-beta.0 | anza-xyz/agave | - | - | - | - |
| 703 | v4.1.0-beta.1 | anza-xyz/agave | 2026-05-22 | - | - | - |
| 704 | v4.1.0-beta.2 | anza-xyz/agave | 2026-06-01 | - | - | - |
| 705 | v4.1.0-beta.3 | anza-xyz/agave | 2026-06-05 | - | - | - |
| 706 | v4.1.0-rc.0 | anza-xyz/agave | 2026-06-12 | - | - | - |
| 707 | v4.1.0-rc.1 | anza-xyz/agave | 2026-06-16 | - | - | - |
| 708 | v4.1.0 | anza-xyz/agave | 2026-06-26 | - | - | - |
| 709 | v4.1.1 | anza-xyz/agave | 2026-07-02 | - | - | - |
| 710 | v4.1.2 | anza-xyz/agave | 2026-07-10 | - | `v4-1` | 429,554,364–438,499,847 |
| 711 | v4.2.0-beta.0 | anza-xyz/agave | 2026-07-06 | - | - | - |
| 712 | v4.2.0-beta.1 | anza-xyz/agave | 2026-07-10 | - | - | - |
| 713 | v4.2.0-beta.2 | anza-xyz/agave | 2026-07-17 | - | - | - |
| 714 | v4.2.0-rc.0 | anza-xyz/agave | 2026-07-24 | - | - | - |
| 715 | v4.2.0-rc.1 | anza-xyz/agave | 2026-07-31 | - | - | - |
| 716 | v4.2.0 | anza-xyz/agave | 2026-08-07 | - | - | - |
| 717 | v4.2.1 | anza-xyz/agave | 2026-08-13 | - | `v4-2` | 438,499,848–451,646,918 |
| 718 | v4.2.2 | anza-xyz/agave | 2026-08-28 | - | - | - |
| 719 | v4.3.0-alpha.1 | anza-xyz/agave | 2026-07-13 | - | - | - |
| 720 | v4.3.0-alpha.2 | anza-xyz/agave | 2026-07-21 | - | - | - |
| 721 | v4.3.0-alpha.3 | anza-xyz/agave | 2026-08-05 | - | - | - |
| 722 | v4.3.0-beta.0 | anza-xyz/agave | 2026-08-14 | - | - | - |
| 723 | v4.3.0-beta.1 | anza-xyz/agave | 2026-08-21 | - | - | - |
| 724 | v4.3.0-beta.2 | anza-xyz/agave | 2026-08-21 | - | - | - |
| 725 | v4.3.0-beta.3 | anza-xyz/agave | 2026-08-28 | - | - | - |
| 726 | v4.3.0-rc.0 | anza-xyz/agave | 2026-09-04 | - | - | - |
| 727 | v4.3.0-rc.1 | anza-xyz/agave | 2026-09-11 | - | - | - |
| 728 | v4.3.0 | anza-xyz/agave | 2026-09-18 | - | - | - |
| 729 | v4.4.0-alpha.2 | anza-xyz/agave | 2026-08-28 | - | - | - |
| 730 | v4.4.0-alpha.3 | anza-xyz/agave | 2026-09-03 | - | - | - |
| 731 | v4.4.0-alpha.4 | anza-xyz/agave | 2026-09-10 | - | - | - |
| 732 | v4.4.0-alpha.5 | anza-xyz/agave | 2026-09-18 | - | - | - |
| 733 | v4.4.0-beta.0 | anza-xyz/agave | 2026-09-28 | - | - | - |
| 734 | v4.5.0-alpha.1 | anza-xyz/agave | 2026-10-03 | - | - | - |
| 735 | v4.5.0-alpha.2 | anza-xyz/agave | 2026-10-08 | - | - | - |

</details>

## 0.2.0 runtime naming

1. **Centralized, provenance-backed naming:** done for the CLI (`capabilities`, the demo), receipts
   (`runtime`), the SDK types and these guides. Open: SVM Replay Studio should show `runtime.name`.
2. **Complete version and coverage matrix with `-` for unsupported versions:** done (above).
3. **Release dates, mainnet activation and reviewed slots kept apart:** release dates and reviewed
   slots are done; mainnet activation slots are open (`-`).
4. **Installed, transaction-qualified and broadly qualified kept apart:** done (above).
5. **Tests for every mapping, unknown runtimes and existing IDs:** done in the registry tests;
   receipts and `--family` keep the existing IDs.

## Coverage gaps

- Reviewed profiles cover mainnet slots 353,951,234–451,646,918; newer slots use
  the explicitly unqualified newest-runtime policy described above.
- 728 of 735 tags are not pinned. Their transactions replay only where their slots fall
  in a pinned runtime's reviewed interval.
- No runtime is broadly qualified, and mainnet activation per release is not tracked.
- Other validator clients are not part of the inventory.
