# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/bipa-app/styled-qr/releases/tag/v0.1.0) - 2026-10-11

### Added

- *(qrcoder)* add const checked constructors for units
- *(qrcoder)* add linear light blending
- *(qrcoder)* add logo plate
- *(qrcoder)* add connected module style
- *(qrcoder)* add scene, raster and SVG renderers
- moving clippy config from isolated script to Cargo.toml

### Fixed

- *(qrcoder)* keep one cache entry per logo size
- *(qrcoder)* check styles once and shrink only for logo errors
- *(qrcoder)* limit format and version damage to what BCH corrects
- *(qrcoder)* keep reused stamps whole and declare SVG blending
- *(qrcoder)* refuse finders and logos that make the symbol unreadable
- *(qrcoder)* bound the logo cache and check the logo size first
- *(qrcoder)* calculate disc coverage with better precision
- fix typos

### Other

- install the crate from crates.io
- add examples and an image gallery to the README
- check format, lints, tests and docs
- add the MIT license and a README
- make the crate standalone as styled-qr
- *(qrcoder)* name presets by their shape
- *(qrcoder)* decode a payload of Lightning invoice length
- *(qrcoder)* remove the Builder
- *(qrcodes)* select QR logos with an enum
- *(qrcoder)* match closed enums with all variants
- *(qrcoder)* cache resized logos
- *(qrcoder)* run Builder on the new renderer
- *(qrcoder)* add symbol model with module roles and codeword map
- *(qrcoder)* add typed units, exact geometry and style types
- Lints | Chore: convert the misc lossless remainder without a cast
- Lints | Chore: remove the `cast_sign_loss` casts outside `src/`
- Lints | Chore: remove the truncating casts in qrcoder
- Lints | Chore: remove the wrap casts in qrcoder
- workspace remaining shared dependencies
- workspace shared dependencies + inline-drift guard
- lopdf and imageproc and cargo update
- *(noop)* remove some more unwraps and panics and support '?'
- base16ct to 1.0, sendgrid to 0.25 and imageprod to 0.26
- update rust
- Revert "Revert "bump: rust to 1.88 stable and let chain stuff""
- Revert "bump: rust to 1.88 stable and let chain stuff"
- rust to 1.88 stable and let chain stuff
- more2024
- thiserror from 1.0 to 2.0
- add expected reason lint
- lint me daddy
- opentelemetry, fe2o3-amqp and image-proc deps
- updates
- Cargo update
- One dockerfile coming right up
- bankly crate no as f32
- Deny clippy::similar-names
- Deny clippy::cast-lossless
- Deny clippy::return-self-not-must-use
- Add central clippy script with configs
- Fix clippy for 1.65
- Upgrade image crate
- clippy stuff
- cargo deny
- Fix clippy complaints on crates
- Add clippy correctness lints to CI
- felipe/bip 428 add qrcode image field to
- Implement qrcode crate
