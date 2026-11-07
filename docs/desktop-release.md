# Desktop release (Linux)

The download is a tarball on GitHub Releases: <https://github.com/shuff57/verSHion/releases/latest>.
The web app links to that page from the landing footer ("Get the desktop app for Linux") and from
Account → Help & community (`DESKTOP_RELEASES` in `src/screens.rs`; the `latest` alias means a new
release needs no code change).

## Build it in Debian 12, not on the dev machine

The dev machine's glibc is 2.43, so a native `cargo build --release` produces a binary that only runs
on the newest distributions. Build in `rust:1-bookworm` (glibc 2.36) to cover Debian 12 and
Ubuntu 22.04 and later:

```
docker run --rm -v "$PWD:/w" -v vershion-desktop-target:/target \
  -e CARGO_TARGET_DIR=/target -e VERSHION_POOL_URL=https://vershion.lefthanddev.com \
  -w /w rust:1-bookworm sh -c 'apt-get update -qq && apt-get install -y -qq mold && cargo build --release -p vershion'
```

`mold` is there because `.cargo/config.toml` links with `-fuse-ld=mold`; without it the build dies at
the first C build script with `collect2: fatal error: cannot find 'ld'`.

Check the result before uploading:

```
objdump -T vershion | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1   # must be 2.36 or lower
```

A native build is fine for a smoke test only: it needs the machine's own glibc.

## Package

Copy the binary out of the volume, then ship it with the README:

```
docker run --rm -v vershion-desktop-target:/target -v "$PWD/out:/out" alpine \
  cp /target/release/vershion /out/vershion
```

The tarball holds `vershion` and `README.txt` (install and run notes, data location, licence) inside
`vershion-<version>-linux-x86_64/`. The app is one self-contained file: fonts, figures and help clips
are compiled in, so there is nothing to install and nothing to keep beside it.

## Publish

```
gh release create v<version> vershion-<version>-linux-x86_64.tar.gz \
  --title "verSHion <version> alpha — Linux desktop build" --notes-file NOTES
```

The version comes from `Cargo.toml` (`0.1.0`) and the tag is `v` + that, so the binary, the tarball
and the release name cannot drift. Tags so far: `v0.1.0` (labelled alpha in the title).

## What a release must say

- Linux x86_64 only; no Windows or macOS build.
- No installer, no auto-update: unpack the newer tarball beside the old data folder.
- Alpha: expect rough edges; issues at <https://github.com/shuff57/verSHion/issues>.
- MIT; bundled fonts under the SIL Open Font License 1.1.
