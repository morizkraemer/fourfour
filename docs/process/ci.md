# CI

Workflow: `.github/workflows/ci.yml` (branch `r63-ci-v2`, rebuilt from `r63-ci` on current `ov-1028`, supersedes `r35-ci`, not yet pushed or run). Triggers: push to `master`, every pull request, manual run (`workflow_dispatch`, use it for a feature branch without a PR). Same-ref runs cancel older ones. No secrets, no signing, no publishing.

Five jobs, no `fail-fast` coupling between them. Node 22, stable Rust, `tauri-cli ~2.10` (matches `tauri 2.10` in `Cargo.lock`).

## Jobs

**linux** (`ubuntu-24.04`, 75 min), in order:

1. App: `npm ci`, `npm test` (vitest), `npm run check` (svelte-check), `npm run build`.
2. Chromium smoke: `npm run smoke` with `FF_SMOKE_SLOWMO_FACTOR=2` (load-scaled timeouts for the shared runner) and `FF_SMOKE_RETRIES=0`. The last output line is `smoke: status <s>, N failed, N flaky, load average X`.
3. `cargo build --workspace`, `cargo test --workspace`. Tests that need weights or real fixtures stay `#[ignore]`d; unix-only tests are `cfg(unix)`. Then `cargo check -p pioneer-test-ui --features dev-bins --bins` so the bench and waveform binaries keep compiling.
4. Install `tauri-cli`, fetch Beat This weights (`fourfour-analysis-rs/scripts/fetch-models.sh`).
5. `fourfour-analysis-rs/scripts/bundle-app.sh --ci --bundles deb`.
6. Deb checks: models and both licences under `lib/fourfour/models/`, no `bench_*` or `waveform_*` binary, `Depends` has `libasound2t64 | libasound2` and `libssl3t64 | libssl3`. Then `dpkg-deb -x` and `pioneer-test-ui --self-check` from the extracted tree (weights resolve under `usr/lib/fourfour/models`).
7. Upload the deb (7 days).

**notices** (`ubuntu-24.04`, 20 min): `npm ci --ignore-scripts` (the script reads licence files from `app/node_modules`), `cargo fetch --locked` (the script runs `cargo metadata --offline` and reads `~/.cargo/registry/src`), then `python3 -I scripts/release/third-party-notices.py` into a temp file and `diff -u` against `pioneer-test-ui/resources/THIRD-PARTY-NOTICES.txt`. Fails when the committed file is stale after a dependency change. It runs on Linux because the script pins `--filter-platform aarch64-apple-darwin` and `cargo fetch` downloads every platform's crates, so the host does not change the result.

**webkit-smoke** (`ubuntu-24.04`, `continue-on-error`): Playwright WebKit with `--with-deps`, `FF_SMOKE_BROWSER=webkit npm run smoke` (same factor 2 and retries 0). Allowed to fail (missing WebKit system libraries on the runner, or WebKit-only differences); a red job here does not fail the run.

**macos** (`macos-14`, Apple Silicon, 90 min): asserts `uname -m` is `arm64`, `npm ci` and `npm run build`, `cargo test --workspace`, `tauri-cli`, then `scripts/mac/build-dmg.sh` (it passes `--features coreml` and `tauri.bundle-weights.conf.json` itself) and `scripts/mac/check-dmg.sh <dmg>` (unsigned or ad-hoc, weights, both licences, Info.plist, `--self-check` of the app binary, which also proves the ONNX Runtime dylib loads). The `coreml` feature pulls `ort` 2.0.0-rc.14 (`download-binaries`), which downloads ONNX Runtime from `cdn.pyke.io` on the first build; the download is cached in `~/Library/Caches/ort.pyke.io`. There is no Intel macOS ONNX Runtime build, so an `x86_64` or universal job would fail; keep this job on arm64. Minimum macOS 13.4 comes from `tauri.conf.json`.

**windows** (`windows-2022`, 90 min, bash shell): finds the preinstalled OpenSSL (or `choco install openssl`) and exports `OPENSSL_DIR`, `OPENSSL_INCLUDE_DIR`, `OPENSSL_LIB_DIR` because `libsqlite3-sys` with `bundled-sqlcipher` panics on Windows without them; `npm ci`, `npm run build`, `cargo test --workspace`, `tauri-cli`, weights, then `bundle-app.sh --ci --bundles nsis`. After that an informational step (`continue-on-error`) installs the NSIS setup silently and runs `--self-check` on the installed exe (checks `<exe dir>/models`).

## Caches

cargo (`Swatinem/rust-cache`), npm, Playwright Chromium (Linux), `cargo-tauri` binary, ONNX Runtime download (macOS), model weights (`prototyping/analysis-packaging/models`, keyed on the fetch script).

## Time

Estimates, not measured. Cold: 30-60 min per OS (debug build, test, release build, `cargo install tauri-cli`; r20 built the deb in about 25 min at load 50). Warm: 12-20 min.

## Reading artifacts

Run page, Artifacts section:

- `fourfour-linux-unsigned`: the `.deb`
- `fourfour-windows-unsigned`: the NSIS `.exe`
- `fourfour-macos-unsigned`: `.dmg`, `.sha256`, `build.log` (contains the weights hashes). Gatekeeper blocks it when downloaded; right-click open, or `xattr -dr com.apple.quarantine`.
- `smoke-results-chromium`, `smoke-results-webkit`: only on a failed smoke run, Playwright traces and screenshots.

The macOS job's `check-dmg.sh` output is in the step log; its last line `failures=0` means pass.

## Known gaps, only a real runner shows them

- Windows link: MSVC build of the C and C++ crates (stretch, sqlcipher amalgamation) and the exact OpenSSL layout on the image (`libcrypto.lib` location is searched, not assumed). Earlier checks were `cargo check` only, with stand-ins.
- Windows shipping: sqlcipher links `libcrypto` dynamically, so the installed app needs `libcrypto-3-x64.dll`. The runner has it on `PATH` through the OpenSSL `bin` dir, which can hide the problem in the self-check step. Bundling the DLL (or vendored OpenSSL) is not done here.
- NSIS silent install path and the installed exe name in the Windows self-check.
- macOS: ORT download from pyke, Xcode clang for `libclang_rt.osx.a`, `hdiutil` in the check script on a hosted runner, `caffeinate`.
- WebKit smoke on Ubuntu 24.04.
- Cache hit rates and real durations.
