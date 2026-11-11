# GPUI Fast runtime for Ely

This independent source branch contains GPUI Fast 0.1.14, generated with upstream's
`script/prepare-release.ts --tag v0.1.14` from
`longbridge/gpui-fast@a427a7d9a6072473ad092b1673cd5235f99284cc`.
It is not the Ely component workspace and must not be merged into its main branch.
The published package and Rust library names are preserved. The release generator
uses registry support snapshots 0.3.12 and wgpu 30.0.1; no GPUI Kit is required.

Narrow native fixes preserve the existing ElyOffice behavior: real quit request
observation in the test platform, fail-fast corrupt GIF/WebP decoding, accessibility
screen bounds and topmost titlebar hit testing, macOS first-responder accessibility,
and Windows language selection, save extensions and safe window teardown.
Source files remain Apache-2.0 under their bundled licenses. These modifications
are integration patches, not upstream release claims. Linux validation and native
macOS/Windows limits are recorded by the integrating Yu migration.

Consumers must pin one full git revision for every Fast package. Upstream Fast is
experimental; a successful compile does not establish editor, IME or platform parity.
