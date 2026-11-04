# The Verus release `make verify` runs, read by tools/cargo-common.sh. It is
# past the seven-day publication cutoff, and gid's verus_builtin_macros
# dependency in Cargo.toml is this release's own; update them together, with
# the toolchain the release was built against and each archive's digest. See
# docs/verification.md.

verus_version=0.2026.10.26.d17fbd0
verus_toolchain=1.99.1

# The files Verus proves, each checked on its own as a crate root.
verus_verified='gid/src/position.rs'

case "$(uname -s)-$(uname -m)" in
    Linux-x86_64)
        verus_platform=x86-linux
        verus_digest=361674d05f0c1b2c9c3f4d5bc56a8fb2ab9241ed0cceca27780668c292bb4940
        ;;
    Darwin-arm64)
        verus_platform=arm64-macos
        verus_digest=5e62be446a5da782d64caeee41bb1490c8aa3c65c0afe380d1c1dcbe207571ff
        ;;
    Darwin-x86_64)
        verus_platform=x86-macos
        verus_digest=24a23b0267f7a225c6ece4bc019930ca273bd57207fe0c1dd3cc7dc5726a37bc
        ;;
    *)
        verus_platform=
        verus_digest=
        ;;
esac
