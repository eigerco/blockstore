# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0](https://github.com/eigerco/blockstore/compare/v0.5.0...v0.6.0) - 2024-06-27

### Added
- add missing store impls & add `len` ([#21](https://github.com/eigerco/blockstore/pull/21))
- [**breaking**] Add `remove` method ([#18](https://github.com/eigerco/blockstore/pull/18))

### Fixed
- *(doc)* rename doc_cfg guard to docsrs, [rust-lang/cargo#13875](https://github.com/rust-lang/cargo/issues/13875) ([#20](https://github.com/eigerco/blockstore/pull/20))

## [0.5.0](https://github.com/eigerco/blockstore/compare/v0.4.0...v0.5.0) - 2024-04-15

### Added
- [**breaking**] Rename `BlockstoreError` to `Error` ([#17](https://github.com/eigerco/blockstore/pull/17))
- Implement `RedbBlockstore` ([#12](https://github.com/eigerco/blockstore/pull/12))
- [**breaking**] Add `BlockstoreError::ValueTooLarge` ([#15](https://github.com/eigerco/blockstore/pull/15))
- [**breaking**] Rename `BlockstoreError::CidTooLong` to `BlockstoreError::CidTooLarge` ([#14](https://github.com/eigerco/blockstore/pull/14))
- [**breaking**] Refine error `BlockstoreError` variants ([#11](https://github.com/eigerco/blockstore/pull/11))

### Other
- Polish before release ([#16](https://github.com/eigerco/blockstore/pull/16))

## [0.4.0](https://github.com/eigerco/blockstore/compare/v0.3.0...v0.4.0) - 2024-04-03

### Other
- Conditionally compile `Send` bounds instead of `SendWrapper` ([#8](https://github.com/eigerco/blockstore/pull/8))

## [0.3.0](https://github.com/eigerco/blockstore/compare/v0.2.0...v0.3.0) - 2024-03-28

### Other
- Use RPITIT instead of async-trait ([#6](https://github.com/eigerco/blockstore/pull/6))

## [0.2.0](https://github.com/eigerco/blockstore/compare/v0.1.1...v0.2.0) - 2024-03-25

### Added
- *(blockstore)* add IndexedDb blockstore ([#221](https://github.com/eigerco/lumina/pull/221))
- feat!(blockstore): add sled blockstore ([#217](https://github.com/eigerco/lumina/pull/217))
- *(blockstore)* Implement LruBlockstore ([#207](https://github.com/eigerco/lumina/pull/207))

### Fixed
- fix!(blockstore): remove an error if cid already exists ([#224](https://github.com/eigerco/lumina/pull/224))

### Other
- *(ci)* add gitignore and CI workflows

## [0.1.1](https://github.com/eigerco/lumina/compare/blockstore-v0.1.0...blockstore-v0.1.1) - 2024-01-15

### Other
- add authors and homepage ([#180](https://github.com/eigerco/lumina/pull/180))

## [0.1.0](https://github.com/eigerco/lumina/releases/tag/blockstore-v0.1.0) - 2024-01-12

### Added
- Add in-memory blockstore ([#160](https://github.com/eigerco/lumina/pull/160))

### Other
- add missing metadata to the toml files ([#170](https://github.com/eigerco/lumina/pull/170))
- document public api ([#161](https://github.com/eigerco/lumina/pull/161))
