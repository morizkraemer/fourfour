# macOS beta install (unsigned)

The beta `.dmg` is not signed or notarised, so Gatekeeper blocks the first launch.
This is expected. Do it once per download.

1. Open the `.dmg` and drag `fourfour.app` into `Applications`.
2. In `Applications`, right-click (or Control-click) `fourfour.app` and choose **Open**.
3. In the dialog, click **Open** again.

If macOS only offers "Move to Trash" or says the app is damaged, remove the
quarantine flag and open it again:

```bash
xattr -dr com.apple.quarantine /Applications/fourfour.app
```

On macOS 15 and later, if the right-click route shows no **Open** button: try
to open the app once, then go to System Settings > Privacy & Security, scroll
to the message about `fourfour`, and click **Open Anyway**.

## Headless check

```bash
/Applications/fourfour.app/Contents/MacOS/fourfour --self-check
```

Opens no window. Loads the bundled analysis weights and prints one line, exit
code 0 when the weights load, 1 otherwise.

## Building the dmg (maintainers)

`scripts/mac/ship-dmg.sh` on the dev box bundles the current branch, builds an
unsigned dmg on the Mac in `~/dev/build/fourfour-dmg` (separate from the dev
install flow), runs `scripts/mac/check-dmg.sh` against it, and copies the log,
SHA256 and check output into `build/mac/dmg-<stamp>/`. The `.dmg` itself is
copied only if it is under 300 MB.
