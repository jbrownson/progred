# Shared by tools/sandbox-cargo and tools/linux-cargo, which source it after
# checking their platform and then define run_sandboxed: Cargo's commands are
# the same on both, and only the sandbox differs. See docs/build-security.md.
#
#   run_sandboxed ALLOW_NETWORK ALLOW_LOCK_WRITE COMMAND [ARGUMENTS...]

project_root=$(dirname "$tool_dir")
build_root="$project_root/target/sandbox"
cargo_home="$build_root/cargo-home"
sandbox_home="$build_root/home"
sandbox_tmp="$build_root/tmp"
target_dir="$build_root/build"
user_name=$(id -un)
wrapper=${0##*/}

rustup_path=$(command -v rustup 2>/dev/null || true)
if [ -z "$rustup_path" ]; then
    for candidate in "$HOME/.cargo/bin/rustup" /opt/homebrew/bin/rustup /usr/local/bin/rustup; do
        if [ -x "$candidate" ]; then
            rustup_path=$candidate
            break
        fi
    done
fi

# A distribution's Cargo serves for stable when there's no rustup.
use_toolchain() {
    if [ -n "$rustup_path" ]; then
        cargo_path=$("$rustup_path" which cargo --toolchain "$1")
    elif [ "$1" = stable ] && command -v cargo >/dev/null 2>&1; then
        cargo_path=$(command -v cargo)
    else
        echo "$wrapper: rustup was not found, so the $1 toolchain is unavailable." >&2
        echo "$wrapper: install rustup in the home directory as web/README.md describes. A distribution compiler covers stable builds until then." >&2
        exit 1
    fi
    toolchain_root=$(dirname "$(dirname "$(readlink -f "$cargo_path")")")
    cargo_path="$toolchain_root/bin/cargo"
    rustc_path="$toolchain_root/bin/rustc"
    rustdoc_path="$toolchain_root/bin/rustdoc"
}
use_toolchain stable

mkdir -p "$cargo_home" "$sandbox_home" "$sandbox_tmp" "$target_dir"

# Build code can't read Git data, so release builds are told which commit they
# came from, since they're what people run and share. Development builds, tests
# included, leave it out: progred's source reads it, so every commit would
# otherwise rebuild progred.
stamp() {
    for arg in "$@"; do
        case $arg in
            --) return ;;
            --release | -r) "$tool_dir/progred-commit"; return ;;
        esac
    done
}

# Updates and resolution exclude registry releases less than seven days old,
# with the nightly the browser build pins, the only Cargo whose resolver has
# the cutoff until Rust 1.100. Resolution runs no dependency code.
use_publish_age_cargo() {
    . "$tool_dir/web-build-settings.sh"
    use_toolchain "$web_toolchain"
}

# Downloads the pinned Verus release with network access, checks the archive
# against its recorded digest, and unpacks it offline, all in the sandbox.
fetch_verus() {
    if [ -z "$verus_platform" ]; then
        echo "$wrapper: no Verus release is pinned for $(uname -s) $(uname -m)" >&2
        exit 1
    fi
    verus_dir="$build_root/verus"
    archive="$verus_dir/verus-$verus_version-$verus_platform.zip"
    staging="$verus_dir/staging"
    mkdir -p "$verus_dir"
    rm -rf "$staging" "$archive"
    echo "$wrapper: downloading Verus $verus_version" >&2
    run_sandboxed 1 0 /usr/bin/curl --fail --location --silent --show-error \
        --output "$archive" \
        "https://github.com/verus-lang/verus/releases/download/release%2F$verus_version/verus-$verus_version-$verus_platform.zip"
    if command -v sha256sum >/dev/null 2>&1; then
        digest=$(sha256sum "$archive")
    else
        digest=$(shasum -a 256 "$archive")
    fi
    if [ "${digest%% *}" != "$verus_digest" ]; then
        echo "$wrapper: the Verus archive's digest is ${digest%% *}, not the pinned $verus_digest" >&2
        rm -f "$archive"
        exit 1
    fi
    run_sandboxed 0 0 unzip -q "$archive" -d "$staging"
    rm -rf "$verus_root"
    mv "$staging/verus-$verus_platform" "$verus_root"
    rm -rf "$staging" "$archive"
}

cargo_command() {
    case ${1-} in
        fetch)
            shift
            run_sandboxed 1 0 "$cargo_path" fetch --locked "$@"
            ;;
        update)
            shift
            use_publish_age_cargo
            run_sandboxed 1 1 "$cargo_path" update -Z min-publish-age \
                --config 'registry.global-min-publish-age="7 days"' "$@"
            ;;
        # After declaring a dependency, record it in Cargo.lock while keeping
        # the versions already locked.
        resolve)
            shift
            use_publish_age_cargo
            run_sandboxed 1 1 "$cargo_path" metadata -Z min-publish-age \
                --config 'registry.global-min-publish-age="7 days"' \
                --format-version 1 "$@" >/dev/null
            ;;
        # The pinned advisory scanner, installed from its lockfile on first
        # use. The installation builds with network access.
        audit)
            shift
            audit_path="$cargo_home/bin/cargo-audit"
            if [ ! -x "$audit_path" ]; then
                run_sandboxed 1 0 "$cargo_path" install cargo-audit \
                    --version '=0.22.2' --locked --no-default-features --root "$cargo_home"
            fi
            run_sandboxed 1 0 "$audit_path" audit --file "$project_root/Cargo.lock" "$@"
            ;;
        # Verus proves the files tools/verus-settings.sh lists. The first run
        # downloads the pinned release with network access and checks its
        # digest; the proofs run offline. Arguments go to every verus run.
        verify)
            shift
            . "$tool_dir/verus-settings.sh"
            use_toolchain "$verus_toolchain"
            verus_root="$build_root/verus/$verus_version"
            if [ ! -x "$verus_root/verus" ]; then
                fetch_verus
            fi
            for file in $verus_verified; do
                echo "verus: $file" >&2
                # The verus launcher would find its toolchain through rustup,
                # which the sandbox hides; it runs the toolchain's own
                # libraries directly instead. env sets the library path inside
                # the sandbox, since macOS drops DYLD_ variables when it
                # starts a system program such as sandbox-exec.
                run_sandboxed 0 0 /usr/bin/env VERUS_USE_RUSTUP=0 \
                    LD_LIBRARY_PATH="$toolchain_root/lib" \
                    DYLD_FALLBACK_LIBRARY_PATH="$toolchain_root/lib" \
                    "$verus_root/verus" --crate-type=lib --edition=2024 \
                    "$project_root/$file" "$@"
            done
            ;;
        # Browser workers rebuild std with atomics; native builds stay stable.
        # The fallback, for pages without shared memory, builds without them.
        web-threaded-fetch | web-threaded | web-fallback)
            web_command=$1
            shift
            . "$tool_dir/web-build-settings.sh"
            use_toolchain "$web_toolchain"
            target_dir="$build_root/build-web"
            if [ "$web_command" = web-fallback ]; then
                web_rustflags=$web_fallback_rustflags
                target_dir="$build_root/build-web-fallback"
            fi
            if [ "$web_command" = web-threaded-fetch ]; then
                run_sandboxed 1 0 "$cargo_path" fetch --locked \
                    --manifest-path "$toolchain_root/lib/rustlib/src/rust/library/sysroot/Cargo.toml" \
                    --target "$web_target"
            else
                run_sandboxed 0 0 "$cargo_path" --frozen "$@" \
                    -Z "build-std=$web_build_std" --target "$web_target"
            fi
            ;;
        "")
            echo "usage: tools/$wrapper audit | fetch | update | resolve | verify | web-threaded-fetch | web-threaded <cargo command> | web-fallback <cargo command> | <cargo command> [arguments...]" >&2
            exit 2
            ;;
        # Builds are offline; fetch the locked graph first if anything is
        # missing. Fetching runs no dependency code.
        *)
            if ! run_sandboxed 0 0 "$cargo_path" fetch --frozen --quiet 2>/dev/null; then
                run_sandboxed 1 0 "$cargo_path" fetch --locked
            fi
            run_sandboxed 0 0 "$cargo_path" --frozen "$@"
            ;;
    esac
}
