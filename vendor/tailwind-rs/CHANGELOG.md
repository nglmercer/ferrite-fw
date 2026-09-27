# Changelog

All notable changes to this pre-1.0 project are documented here. Minor
versions MAY contain breaking API or grammar changes until 1.0.

## [Unreleased]

### Fixed

- Evict semantic candidate cache entries when their last source reference is removed.
- Ignore generated, binary, and common dependency/build directories during CLI collection.
- Process watch events incrementally and remove deleted or renamed source files.
- Normalize Vite module IDs and remove deleted modules from compiler state.
- Publish N-API platform package metadata instead of relying on a private runtime package.

### Changed

- Bumped the transport protocol to version 2 for structured diagnostic severity and help fields.

### Security

- Added a private vulnerability reporting policy and explicit MIT/Apache-2.0 licensing.
