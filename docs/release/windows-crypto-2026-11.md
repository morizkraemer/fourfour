# Windows crypto for SQLCipher (r65, 2026-11-02)

Decision doc for the missing `libcrypto-3` DLL in the Windows build. Research only, no code changed.
`pioneer-usb-writer/Cargo.toml` is the brother's lane, so section 3 is a proposal for him.
Facts below come from the local registry sources (`libsqlite3-sys-0.31.0` `build.rs` and `Cargo.toml`,
`rusqlite-0.33.0`, `openssl-src-300.6.1+3.6.3`) and from `cargo tree`. Unverified items are marked.

## 1. What pulls OpenSSL

**No crate in the build graph pulls OpenSSL on any target.** `cargo tree -i openssl-sys` (also with
`-e all`) fails with "did not match any packages" for `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`
and `x86_64-unknown-linux-gnu`. `openssl-sys` is only an optional dependency of `libsqlite3-sys`,
enabled by the feature `bundled-sqlcipher-vendored-openssl`, which we do not use. (`native-tls` and
`openssl-sys` appear in `Cargo.lock` but are not resolved for these targets.)

The OpenSSL dependency is a plain C link, written by the `libsqlite3-sys` build script
(`bundled-sqlcipher` branch, `build.rs` lines 143-220):

| Target | What the build script does | Result |
|---|---|---|
| Windows (MSVC) | `OPENSSL_LIB_DIR` + `OPENSSL_INCLUDE_DIR` (or `OPENSSL_DIR`) must be set, else it panics ("Missing environment variable OPENSSL_DIR"). Emits `rustc-link-lib=dylib=libcrypto`. | Import lib at build time, `libcrypto-3-x64.dll` needed at run time (DLL name from the OpenSSL 3 install, unverified). Not shipped today. |
| Linux | No env set: emits `dylib=crypto`. Headers from the system. | Dynamic link to system `libcrypto.so`. Deb needs a libssl runtime dependency (how Tauri fills `Depends`: unverified). |
| macOS, native build, no `OPENSSL_DIR`/`OPENSSL_LIB_DIR` in env | `is_apple` branch: `-DSQLCIPHER_CRYPTO_CC`, links frameworks `Security` and `CoreFoundation`. | CommonCrypto. No OpenSSL, nothing to ship. **Confirmed from the build script.** |

macOS caveats: the Apple branch needs `host` and `target` both Apple (no cross-build from Linux), and any
`OPENSSL_DIR` or `OPENSSL_LIB_DIR` in the CI environment (for example a Homebrew export) silently switches it
to OpenSSL dylib linking. Check the macOS job env for those variables (not checked here).

Related fact: `pioneer-library` uses `rusqlite` with plain `bundled`. Cargo unifies features per
`libsqlite3-sys`, so the whole workspace links the SQLCipher amalgamation (4.6.1 community, from
`sqlcipher/sqlite3.c`), not stock SQLite.

## 2. Options

SQLCipher 4.6.1 (amalgamation in the crate) supports these crypto providers: `SQLCIPHER_CRYPTO_CC`,
`..._LIBTOMCRYPT`, `..._NSS`, `..._OPENSSL`, `..._OSSL3` (`sqlite3.c` lines 107083-107087 and 107535-107552).
There is no Windows CNG provider. The crate's build script only wires OpenSSL and CommonCrypto.

### a. Static OpenSSL via `bundled-sqlcipher-vendored-openssl`

`openssl-sys/vendored` builds OpenSSL from source (`openssl-src` 300.6.1+3.6.3) and links it statically.

- **Build needs.** Windows MSVC: a full Perl (Strawberry Perl; the one in Git for Windows is often not enough,
  unverified), MSVC `nmake`, and optionally NASM. `openssl-src` checks for `nasm.exe` and falls back to a
  no-asm build when it is missing (`openssl-src` `lib.rs`; `OPENSSL_RUST_USE_NASM=1/0` forces it). No-asm only
  slows crypto, which matters little here. Whether the `windows-2022` runner image has a usable Perl: unverified.
  Linux and macOS: `perl` and `make`. This box has perl, no nasm.
  Removes the OpenSSL install step and the three `OPENSSL_*` variables from the CI job.
  Needs network at build time for the `openssl-src` crate (not in the local registry before this check).
- **Proof on Linux.** In a scratch copy of the repo (tracked files untouched), `cargo check -p pioneer-usb-writer`
  with the feature changed passed on `x86_64-unknown-linux-gnu`, building OpenSSL 3.6.3 from source:
  4 min 02 s wall with `-j 4` for the whole cold crate graph. Windows cross-check is not possible on this box
  (no MSVC toolchain or linker, perl yes but no nasm); the Windows path is **unverified**.
- **Size.** Static `libcrypto.a` is 60 MB and `libssl.a` 15 MB in a debug build, but the linker pulls only the
  objects SQLCipher references (EVP, HMAC, RAND, PBKDF2 and their dependencies). Estimate +2 to 4 MB in the
  final binary. Not measured. Compare: the dynamic `libcrypto-3-x64.dll` is roughly 5 MB (from memory, unverified).
- **Licence.** OpenSSL 3.x is Apache-2.0 (`openssl-src` ships `LICENSE-APACHE`; `openssl/LICENSE.txt` is the
  Apache-2.0 text). It would now be linked into the binary, so the notice must ship. `openssl-src` and
  `openssl-sys` (MIT/Apache-2.0) only appear in `cargo metadata` if the feature is active for the notices target.
- **Notices script.** `scripts/release/third-party-notices.py` reads crates for `aarch64-apple-darwin` only
  (`TARGET` constant), so a Windows-only dependency would not show up. Add a hand-kept entry for OpenSSL
  (Apache-2.0, `{"standard": "Apache-2.0"}`, holder "The OpenSSL Project Authors") to
  `scripts/release/third-party-extras.json` in the Windows release, or run the script per target.
  The existing SQLCipher component there says "bundled via rusqlite bundled-sqlcipher": update the wording.
- **Pitfall: apply it to Windows only.** In `build.rs` the vendored branch (line 201) is tested before the
  `is_apple` branch, so enabling the feature globally switches macOS from CommonCrypto to static OpenSSL.
  That changes the proven macOS path, adds size and build time, and changes which notices apply. Gate it by
  target (proposal below).

### b. Ship the OpenSSL DLLs as Tauri resources

Copy `libcrypto-3-x64.dll` (from the runner's OpenSSL install or a pinned download) next to the exe.

- **Build needs.** A known OpenSSL install on the runner (the CI job already finds one via choco), plus a
  resource mapping in the Windows Tauri config, for example `"C:/Program Files/OpenSSL-Win64/bin/libcrypto-3-x64.dll": "."`
  (the runner path varies; the CI step already computes `$root`). Tauri puts resources beside the exe on Windows,
  where the loader looks first (resource placement and the exact `"."` target form: unverified, needs a test
  on a runner). `libssl` is not needed for SQLCipher.
- **Size.** About +5 MB in the installer (unverified).
- **Licence.** Apache-2.0 for OpenSSL 3. Which vendor build the DLL comes from (Shining Light via choco, or
  another) is unverified; the notice and licence text still ship as an extras entry.
- **Risk.** The DLL version must match the import library used at link time (`libcrypto.lib` from the same
  install); a mismatch between the job's OpenSSL and the shipped DLL breaks at run time. The CI self-check
  step can hide this because the runner puts OpenSSL `bin` on `PATH` (noted in the r63-ci doc), so the check must
  run with a clean `PATH`.
- **Change in `pioneer-usb-writer/Cargo.toml`:** none.

### c. A different crypto backend

- **CommonCrypto:** Apple only (already used on macOS).
- **Windows CNG / BCrypt:** not a SQLCipher provider (list above). Not possible without a custom provider.
- **LibTomCrypt:** present in the amalgamation (`sqlcipher_ltc_setup`), but `libsqlite3-sys` has no feature for
  it. `build.rs` always demands OpenSSL headers/libs on Windows unless vendored, so using it means a patched or
  forked `libsqlite3-sys` (plus a LibTomCrypt build and link). Not worth the maintenance for a solo project.
- **NSS:** same problem, worse.
- **`SQLCIPHER_CRYPTO_OSSL3`:** exists in the source, but is still OpenSSL.

### d. Other

1. **Static link of a prebuilt OpenSSL** (vcpkg `openssl:x64-windows-static`) with the plain feature: the build
   script emits `dylib=` for `libcrypto` unconditionally, so this does not produce a static link. No.
2. **Defer Windows.** `beta-checklist-2026-11.md` already says Windows is out of scope for the macOS beta. Zero
   work now; the Windows job stays informational. Valid until a Windows release is planned.
3. **`openssl-probe`/`native-tls` entries in `Cargo.lock`:** not linked, ignore.

## 3. Proposal for the brother (`pioneer-usb-writer/Cargo.toml`)

For option (a), Windows only. Keep line 18 as is and add:

```toml
[target.'cfg(windows)'.dependencies]
rusqlite = { version = "0.33", features = ["bundled-sqlcipher-vendored-openssl"] }
```

With `resolver = "2"` (set in the workspace) the extra feature should apply to Windows targets only, so macOS
keeps CommonCrypto and Linux keeps system `libcrypto`. Whether Cargo resolves it as intended here was **not
tested** (only the all-targets variant was checked on Linux): please run
`cargo tree -e features -i openssl-sys --target aarch64-apple-darwin` (must still fail) and the same with
`--target x86_64-pc-windows-msvc` (must show `openssl-sys` with `vendored`).
If Linux should also drop its libssl runtime dependency, use the unconditional form instead (line 18 with
`bundled-sqlcipher-vendored-openssl`), but then macOS also moves to static OpenSSL; only do that on purpose.
Option (b) needs no Cargo change.

## 4. Recommendation

**Do (a), gated to Windows**, when a Windows release is planned: one dependency line, no DLL to ship or
version-match, no extra install step in CI (only Perl, plus NASM optionally). Keeps macOS on CommonCrypto.
Until then, **defer** (d.2): Windows is not in the macOS beta. Use (b) only if a Windows build is needed
before the brother can look at (a). Skip (c).

## 5. Unverified

- Windows MSVC build of vendored OpenSSL (Perl on `windows-2022`, build time, nasm fallback behaviour at run time).
- Real binary size delta of (a) and the DLL size in (b); both are estimates.
- Target-gated feature resolution (`cfg(windows)` block) was not executed.
- Tauri resource placement of the DLL next to the exe, and the exact DLL name from the choco OpenSSL build.
- How the Linux deb gets its libssl dependency; whether any macOS CI env var forces OpenSSL over CommonCrypto.
- A cross-check of the final macOS binary (`otool -L`) for absence of libcrypto; the conclusion comes from the build script only.
