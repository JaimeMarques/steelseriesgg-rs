# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Releases and this changelog are generated automatically from
[Conventional Commits](https://www.conventionalcommits.org/) by release-please.

## [0.1.6](https://github.com/JaimeMarques/steelseriesgg-rs/compare/v0.1.5...v0.1.6) (2026-09-29)


### Added

* add animated Electron device and audio console ([8a83852](https://github.com/JaimeMarques/steelseriesgg-rs/commit/8a838522df69782702243c73c88e77f8988f2052))
* add Apex Pro TKL 2023 Wireless (PID 0x1632) RGB support ([#163](https://github.com/JaimeMarques/steelseriesgg-rs/issues/163)) ([28377ef](https://github.com/JaimeMarques/steelseriesgg-rs/commit/28377ef1b687d43394e855afc838bacf40b74375))
* add bounded desktop audio RPC service with isolated Pulse tests ([de37aef](https://github.com/JaimeMarques/steelseriesgg-rs/commit/de37aefd86ffad94e396b11053be33ecc8ef5298))
* add dedicated Arctis Nova 7 Gen 2 support ([fb4f328](https://github.com/JaimeMarques/steelseriesgg-rs/commit/fb4f328595ad977139dc48f343fef561606dae4c))
* add dedicated Nova 7 Gen 2 protocol and safe capability gates ([f7505dd](https://github.com/JaimeMarques/steelseriesgg-rs/commit/f7505dd54b41cb57232274ac52f0689e25f83801))
* add key mapping verification CLI tool ([#87](https://github.com/JaimeMarques/steelseriesgg-rs/issues/87)) ([4370fca](https://github.com/JaimeMarques/steelseriesgg-rs/commit/4370fcae06ca79ee6d78038599557880f0138699))
* add Linux audio service and physical ChatMix ([7f34650](https://github.com/JaimeMarques/steelseriesgg-rs/commit/7f346507f3bb5311ebad4d88e77ffa46dfc41519))
* add original SSGG app and tray icon family ([ac68762](https://github.com/JaimeMarques/steelseriesgg-rs/commit/ac687623186694be4e369a6a3fb265865c4cad7c))
* add original SSGG icon family and reproducible renders ([5a5ccb8](https://github.com/JaimeMarques/steelseriesgg-rs/commit/5a5ccb8b8ccab6e31338df60856c56a9562c3910))
* add original SSGG icon family and reproducible renders ([2726b2e](https://github.com/JaimeMarques/steelseriesgg-rs/commit/2726b2eb9e34633b75f21b8a86c15efc38a79086))
* add PIDs 0x2290 (Arctis Nova Pro Omni) and 0x1630 (Apex Pro TKL… ([#244](https://github.com/JaimeMarques/steelseriesgg-rs/issues/244)) ([d74544e](https://github.com/JaimeMarques/steelseriesgg-rs/commit/d74544ef213eddb4b9b8a9a8acc1af6d9464e142))
* apply all open PRs — security, perf, tests, refactor, deps ([#226](https://github.com/JaimeMarques/steelseriesgg-rs/issues/226)) ([edb44fe](https://github.com/JaimeMarques/steelseriesgg-rs/commit/edb44fe4da04a9d69bc8751f97f10e97e88db089))
* build sandboxed Electron audio and device console ([e7cc8f6](https://github.com/JaimeMarques/steelseriesgg-rs/commit/e7cc8f604be105cabd94ca96ebeffc47be1f3a3a))
* **desktop:** add source-gated one-shot RGB lighting controls ([db276a6](https://github.com/JaimeMarques/steelseriesgg-rs/commit/db276a6c610cf39e2443c602e4fe4301581ad2b8))
* integrate opt-in Nova 7 Gen 2 physical ChatMix worker ([5c9ac35](https://github.com/JaimeMarques/steelseriesgg-rs/commit/5c9ac355793a7a2bd43a87061fb3ec0a0118f3af))
* package the integrated SSGG desktop for Ubuntu ([364edbc](https://github.com/JaimeMarques/steelseriesgg-rs/commit/364edbc6f39a7fe7f130829d7f29e089e6f0df47))
* **release:** adopt Keep a Changelog and automate releases with release-please ([1e2c81a](https://github.com/JaimeMarques/steelseriesgg-rs/commit/1e2c81a2ee7cbd083bd9c4a7bbbbc42e73328aba))
* **release:** keep PKGBUILD pkgver in sync via release-please ([95774b7](https://github.com/JaimeMarques/steelseriesgg-rs/commit/95774b71e19d6a776d3708d87770568659746c71))


### Fixed

* **apex-2023:** include WIRELESS_2 (0x1630) in per-zone buffer path ([996d5e7](https://github.com/JaimeMarques/steelseriesgg-rs/commit/996d5e7bef1e27d338fb38f4dd1531ee9bbf2a79))
* **build:** disable global sccache wrapper, unavailable in CI ([6d51f38](https://github.com/JaimeMarques/steelseriesgg-rs/commit/6d51f38aa00aa85fd1dbfb5b7efad2a7cf443b73))
* cancel queued physical audio writes after headset disconnect ([9441087](https://github.com/JaimeMarques/steelseriesgg-rs/commit/9441087144b34b840d5bc5666afb50f11418be2e))
* **ci:** address PR review on release automation ([efdb349](https://github.com/JaimeMarques/steelseriesgg-rs/commit/efdb34983bdfd1aa49bd4fe01390daee3f52c31f))
* **ci:** create tag and release on manual dispatch when tag is missing ([434a9cd](https://github.com/JaimeMarques/steelseriesgg-rs/commit/434a9cd39b640029653e81cc5fcadeb6b54abf44))
* **ci:** push release-please tag with a PAT so release.yml triggers ([bdb29a4](https://github.com/JaimeMarques/steelseriesgg-rs/commit/bdb29a4caae176cbee478127389a0d83878f880c))
* **deps:** update all dependencies ([aafd6dc](https://github.com/JaimeMarques/steelseriesgg-rs/commit/aafd6dc6f143a7ddfb75f4b08906a97e5323dc5e))
* **deps:** update all dependencies ([fe1724c](https://github.com/JaimeMarques/steelseriesgg-rs/commit/fe1724c7f0d088475b2a90437793bd229b1b9847))
* **hid:** implement send_feature_report for non-Unix platforms via hidapi ([a942e92](https://github.com/JaimeMarques/steelseriesgg-rs/commit/a942e9279e9192a478131004fc10336a2df24b2b))
* implement robust CORS origin validation ([0c2a343](https://github.com/JaimeMarques/steelseriesgg-rs/commit/0c2a34394c145b2131754205f4b8f30550513841))
* keep hardware shutdown implementation before tests ([3934e4c](https://github.com/JaimeMarques/steelseriesgg-rs/commit/3934e4c7538b97496c9a8323dc095ec5527acf09))
* **keyboards:** make RGB work on Apex Pro TKL Gen 3 (2024) ([#290](https://github.com/JaimeMarques/steelseriesgg-rs/issues/290)) ([360b6c6](https://github.com/JaimeMarques/steelseriesgg-rs/commit/360b6c6d7fc7fb6c9636a44ead65c08a1c67cccd))
* make headset wheel setup discoverable in Mixer ([88eda6d](https://github.com/JaimeMarques/steelseriesgg-rs/commit/88eda6d31054eaadbfcacf79d6afa828e56e23b8))
* **pkgbuild:** reference $srcdir directly in build functions ([c27c030](https://github.com/JaimeMarques/steelseriesgg-rs/commit/c27c030ecd357ff20df1d4eafe31a9e715cf66dd))
* **pkgbuild:** resolve cargo paths at build time, not parse time ([3f4fa96](https://github.com/JaimeMarques/steelseriesgg-rs/commit/3f4fa96b2e5e0daa0e6b9ffe9966b41bcef0ff4a))
* poll Gen 2 wheel status interactively without idle state writes ([770fbd2](https://github.com/JaimeMarques/steelseriesgg-rs/commit/770fbd207a09c813806b15787fc854babf173322))
* **pollrate:** pass null_mut() to CreateFileW hTemplate parameter ([c2358cf](https://github.com/JaimeMarques/steelseriesgg-rs/commit/c2358cff5e6407209994f8342e50f8eb92fd353c))
* preserve HID failure when old status expires ([8ea2180](https://github.com/JaimeMarques/steelseriesgg-rs/commit/8ea218081ae8be4d4cab86775ab677497193e7f0))
* re-query lost Nova status within bounded deadline ([a26bd3e](https://github.com/JaimeMarques/steelseriesgg-rs/commit/a26bd3e2a27c7c30260a4cf65d02f83cb3ab2f45))
* recover Nova wheel status without dropping ownership ([a4185e7](https://github.com/JaimeMarques/steelseriesgg-rs/commit/a4185e74871af3ea59e4570c8ac1e0a2d33dad79))
* **release:** create tag via GitHub API on workflow_dispatch ([16b8f44](https://github.com/JaimeMarques/steelseriesgg-rs/commit/16b8f445acf6cf12d66ec7abf9225bfcfb273d85))
* **release:** handle workflow_dispatch versioning and use resolved version throughout ([69f26bd](https://github.com/JaimeMarques/steelseriesgg-rs/commit/69f26bd3a078c961c9ca54c15c8fa80fea461033))
* remove needless borrow in write_core_props (clippy::needless_borrow) ([df80739](https://github.com/JaimeMarques/steelseriesgg-rs/commit/df807390937f1c4141cc1a6a03981fdbad655c82))
* resolve compilation errors in multiple modules ([#56](https://github.com/JaimeMarques/steelseriesgg-rs/issues/56)) ([893b026](https://github.com/JaimeMarques/steelseriesgg-rs/commit/893b02659cee47773d70598b4850aaa352a81dc6))
* resolve duplicate import and async trait compilation errors ([#72](https://github.com/JaimeMarques/steelseriesgg-rs/issues/72)) ([7ca1f6c](https://github.com/JaimeMarques/steelseriesgg-rs/commit/7ca1f6c646237e6926c2cf6558f893c37dc52d67))
* resolve Windows compilation errors and update per-key RGB API ([99f42e2](https://github.com/JaimeMarques/steelseriesgg-rs/commit/99f42e2cc309f576e1ba3f2b615b14a0da372eb4))
* **snap:** preserve runtime layout and enforce ABI checks across locales ([f175bec](https://github.com/JaimeMarques/steelseriesgg-rs/commit/f175becd6152c8b8c5b20660e7c3101d7732ff3c))


### Changed

* audit and modernize dependencies, remove dead code ([#50](https://github.com/JaimeMarques/steelseriesgg-rs/issues/50)) ([e5f94d1](https://github.com/JaimeMarques/steelseriesgg-rs/commit/e5f94d13c5c997efc03ae27d3d9e5155660f64d2))
* fix synchronous file open blocking async diagnostics ([3cc60ef](https://github.com/JaimeMarques/steelseriesgg-rs/commit/3cc60ef78c1655689ab28c536a4fcab82663e23a))
* implement comprehensive optimization plan ([#51](https://github.com/JaimeMarques/steelseriesgg-rs/issues/51)) ([37a854e](https://github.com/JaimeMarques/steelseriesgg-rs/commit/37a854e5a17cb94c30dc1a6a523ac1b4379ff7b8))
* resolve audio hang, migrate to rustix, update docs ([fae39d9](https://github.com/JaimeMarques/steelseriesgg-rs/commit/fae39d90e22bfcea327780395d461024b4de4035))
* Use dynamic binary name in pollrate error message ([#74](https://github.com/JaimeMarques/steelseriesgg-rs/issues/74)) ([99d2f75](https://github.com/JaimeMarques/steelseriesgg-rs/commit/99d2f75ca60ff0590075932fd42c55c4fde4f08b))


### Documentation

* add database-schemas.md (merged engine + prism + sonar schemas) ([9054ce6](https://github.com/JaimeMarques/steelseriesgg-rs/commit/9054ce6af104b2aa7d93fdb227882b47c78017bb))
* add protocol-keyboard.md with confirmed/experimental command reference ([8271933](https://github.com/JaimeMarques/steelseriesgg-rs/commit/8271933d6a14ab8cf487ad766190928a9630c618))
* add staleness note for Issue [#6](https://github.com/JaimeMarques/steelseriesgg-rs/issues/6) in PLAN.md ([142fe5a](https://github.com/JaimeMarques/steelseriesgg-rs/commit/142fe5af220a6074ac2c68e5fc8768bebcc8345c))
* clarify unverified protocol status and zone mapping for Apex 3 TKL ([07d0139](https://github.com/JaimeMarques/steelseriesgg-rs/commit/07d013966ed76f7254de07471f3f425edc49ef0f))
* consolidate database schemas into single reference ([32eb2c2](https://github.com/JaimeMarques/steelseriesgg-rs/commit/32eb2c22b192033bddf700178d5b19e993fec7be))
* consolidate development docs from 10 files to 4 ([2d1f3e1](https://github.com/JaimeMarques/steelseriesgg-rs/commit/2d1f3e10ebcbc991c68a8e4c1362366b40783fe7))
* Create comprehensive AGENTS.md with symlinks and optimize Copil… ([#46](https://github.com/JaimeMarques/steelseriesgg-rs/issues/46)) ([09c18e6](https://github.com/JaimeMarques/steelseriesgg-rs/commit/09c18e637af1cef4c1fca8fc6be512168492769f))
* escape lone backtick in zone map table cell ([5f33498](https://github.com/JaimeMarques/steelseriesgg-rs/commit/5f33498b2b55696b9df8263162a50ed6444fb6f3))
* fix agnix errors and reduce warnings to 1 ([9ddf684](https://github.com/JaimeMarques/steelseriesgg-rs/commit/9ddf6849a2f84ae6d863fa421519e1383df78518))
* fix agnix errors and reduce warnings to 1 ([58b8bd9](https://github.com/JaimeMarques/steelseriesgg-rs/commit/58b8bd9301ad6d4fdb4f236de148a695e4451388))
* fix corrupted table separator rows in database-schemas.md ([cb17b17](https://github.com/JaimeMarques/steelseriesgg-rs/commit/cb17b173beac9e93bf535d5d04ef2a8f758562f9))
* fix trailing pipe in table separators, clarify PID encoding; add git allowlist to project settings ([e309216](https://github.com/JaimeMarques/steelseriesgg-rs/commit/e309216e73a7eb1b0a7cdbcc2b4f7a10e54c6165))
* mark Phase 1 and Phase 2 complete in PLAN.md and TODO.md ([2f120c1](https://github.com/JaimeMarques/steelseriesgg-rs/commit/2f120c1f773e540c20d37507a4ab2bbe0cf5c41b))
* prune stale tasks from TODO/PLAN, refocus on Apex 3 TKL RGB ([83f7704](https://github.com/JaimeMarques/steelseriesgg-rs/commit/83f7704f5d4c5cd61e91611fb09f690e8befdadf))
* reconcile protocol docs and add experimental qualifiers (Phase 6 & 7) ([0750b49](https://github.com/JaimeMarques/steelseriesgg-rs/commit/0750b498191edfbdab11e6e91f0744a2f172ff5a))
* record desktop product requirements ([d9f77e5](https://github.com/JaimeMarques/steelseriesgg-rs/commit/d9f77e5e0ab373fbeabe9c076cc1673de5a0d1cd))
* remove device-pid-registry.md (merged into devices.md) ([017a800](https://github.com/JaimeMarques/steelseriesgg-rs/commit/017a80076f9c13d595a8d6b5cde28c55b309af52))
* remove device-registry.md (merged into devices.md) ([625458a](https://github.com/JaimeMarques/steelseriesgg-rs/commit/625458af55ec1296ec98b2fcf0f0c7c3ef5c5789))
* remove engine-db-schema.md (merged into database-schemas.md) ([bcc439f](https://github.com/JaimeMarques/steelseriesgg-rs/commit/bcc439f2bb28dfdeef1b1feb343ca74fe55b660b))
* remove gg-internal-api.md (GG internals ssgg doesn't use) ([2007f33](https://github.com/JaimeMarques/steelseriesgg-rs/commit/2007f33397bdc8315d36d4d38834b05859ca1182))
* remove gg-reflection.txt (raw dump, superseded) ([4d8e2e9](https://github.com/JaimeMarques/steelseriesgg-rs/commit/4d8e2e937ce71327a48df86ad17ec00d488836d3))
* remove preflight-findings.md (findings captured in other docs) ([9abd786](https://github.com/JaimeMarques/steelseriesgg-rs/commit/9abd7864fe778dfc7391debf97f51f9e0d229adb))
* remove prism-schema.md (merged into database-schemas.md) ([167560a](https://github.com/JaimeMarques/steelseriesgg-rs/commit/167560aa2dbb9c41fa2a407c6404be2d85e4fbb3))
* remove sonar-schema.md (merged into database-schemas.md) ([1aeac24](https://github.com/JaimeMarques/steelseriesgg-rs/commit/1aeac24fdd09cddd157313c3fbf36c694c36d881))
* reorganize and expand AGENTS.md with structured module layout ([f504d5e](https://github.com/JaimeMarques/steelseriesgg-rs/commit/f504d5ed0beb1b8ec9a52815e4f82e6d92c0daa9))
* replace CLI-first onboarding with desktop installation and usage guides ([233e77d](https://github.com/JaimeMarques/steelseriesgg-rs/commit/233e77d35916cf99a2b82e6a348a9f94b071ba51))
* update AGENTS.md with accurate file map and fix ctxlint errors ([67fb20a](https://github.com/JaimeMarques/steelseriesgg-rs/commit/67fb20a834a37379fe132db4a9e74c82b1245e04))
* update AI coding guardrails and architecture rules ([#123](https://github.com/JaimeMarques/steelseriesgg-rs/issues/123)) ([7e30ba6](https://github.com/JaimeMarques/steelseriesgg-rs/commit/7e30ba6733034639d12db4ac90a48b16ad0a3772))
* update CLAUDE.md/AGENTS.md with comprehensive codebase document… ([#124](https://github.com/JaimeMarques/steelseriesgg-rs/issues/124)) ([a87927d](https://github.com/JaimeMarques/steelseriesgg-rs/commit/a87927dddcb7ea5ea10100f49582bcc0c5d24c75))
* update development research docs to match current implementation ([#114](https://github.com/JaimeMarques/steelseriesgg-rs/issues/114)) ([c4bda02](https://github.com/JaimeMarques/steelseriesgg-rs/commit/c4bda024c98dcb6e8ac95f0bbdda37c74f31b6fe))
* use the corrected desktop 0.1.1 installer ([e0befc4](https://github.com/JaimeMarques/steelseriesgg-rs/commit/e0befc4b696ee0926e4fdca476a8c9672709211b))

## [0.1.5](https://github.com/Ven0m0/steelseriesgg-rs/compare/v0.1.4...v0.1.5) (2026-08-26)


### Fixed

* **keyboards:** make RGB work on Apex Pro TKL Gen 3 (2024) ([#290](https://github.com/Ven0m0/steelseriesgg-rs/issues/290)) ([360b6c6](https://github.com/Ven0m0/steelseriesgg-rs/commit/360b6c6d7fc7fb6c9636a44ead65c08a1c67cccd))

## [0.1.4](https://github.com/Ven0m0/steelseriesgg-rs/compare/v0.1.3...v0.1.4) (2026-08-18)


### Fixed

* **build:** disable global sccache wrapper, unavailable in CI ([6d51f38](https://github.com/Ven0m0/steelseriesgg-rs/commit/6d51f38aa00aa85fd1dbfb5b7efad2a7cf443b73))

## [0.1.3](https://github.com/Ven0m0/steelseriesgg-rs/compare/v0.1.2...v0.1.3) (2026-08-18)


### Fixed

* **ci:** create tag and release on manual dispatch when tag is missing ([434a9cd](https://github.com/Ven0m0/steelseriesgg-rs/commit/434a9cd39b640029653e81cc5fcadeb6b54abf44))

## [0.1.2](https://github.com/Ven0m0/steelseriesgg-rs/compare/v0.1.1...v0.1.2) (2026-07-03)


### Changed

* fix synchronous file open blocking async diagnostics ([3cc60ef](https://github.com/Ven0m0/steelseriesgg-rs/commit/3cc60ef78c1655689ab28c536a4fcab82663e23a))

## [0.1.1](https://github.com/Ven0m0/steelseriesgg-rs/compare/v0.1.0...v0.1.1) (2026-06-19)


### Added

* **release:** adopt Keep a Changelog and automate releases with release-please ([1e2c81a](https://github.com/Ven0m0/steelseriesgg-rs/commit/1e2c81a2ee7cbd083bd9c4a7bbbbc42e73328aba))
* **release:** keep PKGBUILD pkgver in sync via release-please ([95774b7](https://github.com/Ven0m0/steelseriesgg-rs/commit/95774b71e19d6a776d3708d87770568659746c71))


### Fixed

* **ci:** address PR review on release automation ([efdb349](https://github.com/Ven0m0/steelseriesgg-rs/commit/efdb34983bdfd1aa49bd4fe01390daee3f52c31f))
* **ci:** push release-please tag with a PAT so release.yml triggers ([bdb29a4](https://github.com/Ven0m0/steelseriesgg-rs/commit/bdb29a4caae176cbee478127389a0d83878f880c))
* **deps:** update all dependencies ([aafd6dc](https://github.com/Ven0m0/steelseriesgg-rs/commit/aafd6dc6f143a7ddfb75f4b08906a97e5323dc5e))
* **pkgbuild:** reference $srcdir directly in build functions ([c27c030](https://github.com/Ven0m0/steelseriesgg-rs/commit/c27c030ecd357ff20df1d4eafe31a9e715cf66dd))
* **pkgbuild:** resolve cargo paths at build time, not parse time ([3f4fa96](https://github.com/Ven0m0/steelseriesgg-rs/commit/3f4fa96b2e5e0daa0e6b9ffe9966b41bcef0ff4a))
* remove needless borrow in write_core_props (clippy::needless_borrow) ([df80739](https://github.com/Ven0m0/steelseriesgg-rs/commit/df807390937f1c4141cc1a6a03981fdbad655c82))


### Documentation

* add database-schemas.md (merged engine + prism + sonar schemas) ([9054ce6](https://github.com/Ven0m0/steelseriesgg-rs/commit/9054ce6af104b2aa7d93fdb227882b47c78017bb))
* clarify unverified protocol status and zone mapping for Apex 3 TKL ([07d0139](https://github.com/Ven0m0/steelseriesgg-rs/commit/07d013966ed76f7254de07471f3f425edc49ef0f))
* consolidate database schemas into single reference ([32eb2c2](https://github.com/Ven0m0/steelseriesgg-rs/commit/32eb2c22b192033bddf700178d5b19e993fec7be))
* consolidate development docs from 10 files to 4 ([2d1f3e1](https://github.com/Ven0m0/steelseriesgg-rs/commit/2d1f3e10ebcbc991c68a8e4c1362366b40783fe7))
* escape lone backtick in zone map table cell ([5f33498](https://github.com/Ven0m0/steelseriesgg-rs/commit/5f33498b2b55696b9df8263162a50ed6444fb6f3))
* fix agnix errors and reduce warnings to 1 ([9ddf684](https://github.com/Ven0m0/steelseriesgg-rs/commit/9ddf6849a2f84ae6d863fa421519e1383df78518))
* fix agnix errors and reduce warnings to 1 ([58b8bd9](https://github.com/Ven0m0/steelseriesgg-rs/commit/58b8bd9301ad6d4fdb4f236de148a695e4451388))
* fix corrupted table separator rows in database-schemas.md ([cb17b17](https://github.com/Ven0m0/steelseriesgg-rs/commit/cb17b173beac9e93bf535d5d04ef2a8f758562f9))
* fix trailing pipe in table separators, clarify PID encoding; add git allowlist to project settings ([e309216](https://github.com/Ven0m0/steelseriesgg-rs/commit/e309216e73a7eb1b0a7cdbcc2b4f7a10e54c6165))
* prune stale tasks from TODO/PLAN, refocus on Apex 3 TKL RGB ([83f7704](https://github.com/Ven0m0/steelseriesgg-rs/commit/83f7704f5d4c5cd61e91611fb09f690e8befdadf))
* remove device-pid-registry.md (merged into devices.md) ([017a800](https://github.com/Ven0m0/steelseriesgg-rs/commit/017a80076f9c13d595a8d6b5cde28c55b309af52))
* remove device-registry.md (merged into devices.md) ([625458a](https://github.com/Ven0m0/steelseriesgg-rs/commit/625458af55ec1296ec98b2fcf0f0c7c3ef5c5789))
* remove engine-db-schema.md (merged into database-schemas.md) ([bcc439f](https://github.com/Ven0m0/steelseriesgg-rs/commit/bcc439f2bb28dfdeef1b1feb343ca74fe55b660b))
* remove gg-internal-api.md (GG internals ssgg doesn't use) ([2007f33](https://github.com/Ven0m0/steelseriesgg-rs/commit/2007f33397bdc8315d36d4d38834b05859ca1182))
* remove gg-reflection.txt (raw dump, superseded) ([4d8e2e9](https://github.com/Ven0m0/steelseriesgg-rs/commit/4d8e2e937ce71327a48df86ad17ec00d488836d3))
* remove preflight-findings.md (findings captured in other docs) ([9abd786](https://github.com/Ven0m0/steelseriesgg-rs/commit/9abd7864fe778dfc7391debf97f51f9e0d229adb))
* remove prism-schema.md (merged into database-schemas.md) ([167560a](https://github.com/Ven0m0/steelseriesgg-rs/commit/167560aa2dbb9c41fa2a407c6404be2d85e4fbb3))
* remove sonar-schema.md (merged into database-schemas.md) ([1aeac24](https://github.com/Ven0m0/steelseriesgg-rs/commit/1aeac24fdd09cddd157313c3fbf36c694c36d881))
* reorganize and expand AGENTS.md with structured module layout ([f504d5e](https://github.com/Ven0m0/steelseriesgg-rs/commit/f504d5ed0beb1b8ec9a52815e4f82e6d92c0daa9))
* update AGENTS.md with accurate file map and fix ctxlint errors ([67fb20a](https://github.com/Ven0m0/steelseriesgg-rs/commit/67fb20a834a37379fe132db4a9e74c82b1245e04))

## [0.1.0](https://github.com/Ven0m0/steelseriesgg-rs/releases/tag/v0.1.0) (2026-05-31)

Initial release of the open-source SteelSeries GG replacement for Linux.

### Added

- Type-safe HID report builder for talking to SteelSeries devices instead of
  hand-written byte arrays.
- RGB lighting control: colors, effects, per-key effects, and zone-to-HID
  mapping.
- Apex keyboard protocol implementation and headset protocol support.
- Device discovery with hot-plug detection and device fingerprinting.
- GameSense-compatible HTTP server on port 27301 with a localhost-only CORS
  policy.
- Profile save/load as TOML and `~/.config/ssgg/config.toml` configuration.
- Runtime diagnostics, RGB validation, and performance management.
- Optional `audio` feature: PulseAudio/PipeWire mixer.
- Optional `sonar` feature: SteelSeries Sonar HTTP integration.
- Experimental `experimental-apex-2023` feature for Apex Pro TKL 2023 direct
  per-key RGB (reverse-engineered, unverified on hardware).
- udev rules and a systemd user unit for installation.
