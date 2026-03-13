# SACAD

## Smart Automatic Cover Art Downloader

[![CI status](https://img.shields.io/github/actions/workflow/status/desbma/sacad/ci.yml)](https://github.com/desbma/sacad/actions)
[![crates.io version](https://img.shields.io/crates/v/sacad)](https://crates.io/crates/sacad)
[![AUR version](https://img.shields.io/aur/version/sacad.svg?style=flat)](https://aur.archlinux.org/packages/sacad/)
[![License](https://img.shields.io/github/license/desbma/sacad.svg?style=flat)](https://github.com/desbma/sacad/blob/master/LICENSE)

---

Since version 3.0, this tool has been completely rewritten in Rust.

The previous Python version can be found in the [2.x branch](https://github.com/desbma/sacad/tree/2.x).

---

SACAD is a multi platform command line tool to download album covers without manual intervention, ideal for integration in scripts, audio players, etc.

SACAD also provides a second command line tool, `sacad_r`, to scan a music library, read metadata from audio tags, and download missing covers automatically, optionally embedding the image into audio audio files.

## Features

- Can target specific image size, and find results for high resolution covers
- Support JPEG and PNG formats
- Customizable output: save image along with the audio files / in a different directory named by artist/album / embed cover in audio files...
- Currently support the following cover sources:
  - CoverArtArchive (MusicBrainz)
  - Deezer
  - Discogs
  - Last.fm
  - iTunes
  - Qobuz
- Smart sorting algorithm to select THE best cover for a given query, using several factors: source reliability, image format, image size, image similarity with reference cover, etc.
- Automatically crunch PNG images (can save 30% of file size without any loss of quality)
- Cache search data locally for faster future search
- Automatically convert/resize image if needed
- Multi platform (Windows/Mac/Linux)

## Installation

### Binaries

Windows, Mac OS, and Linux binaries are available [on GitHub](https://github.com/desbma/sacad/releases).

### From [`crates.io`](https://crates.io/)

```bash
cargo install sacad --version '>=3.0.0-b.1'
```

### From source

You need a Rust build environment. Install it with [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustup default stable
```

> **macOS note:** Before building you must accept the Xcode license (required for the C linker). Run `sudo xcodebuild -license`, scroll to the bottom, and type `agree`.

Then build and install from the repository:

```bash
cargo install --path .
```

This places the `sacad` and `sacad_r` binaries in `~/.cargo/bin/`, which is added to your `PATH` by rustup.

#### Qobuz source

The Qobuz source requires a valid `app_id`. The value hardcoded in `src/source/qobuz.rs` may become stale over time. To obtain a current one:

1. Open [play.qobuz.com](https://play.qobuz.com) in a browser and search for an album
2. Open DevTools → Network tab and find a request to `api.json/0.2/album/search`
3. Copy the `X-App-Id` value from the request headers
4. Update the `APP_ID` constant in `src/source/qobuz.rs` and rebuild

## Command line usage

Two tools are provided: `sacad` to search and download one cover, and `sacad_r` to scan a music library and download all missing covers.

Run `sacad -h` / `sacad_r -h` to get full command line reference.

### Examples

To download the cover of _Master of Puppets_ from _Metallica_, to the file `AlbumArt.jpg`, targeting ~ 600x600 pixel resolution:

```bash
sacad "metallica" "master of puppets" 600 AlbumArt.jpg
```

To download covers for your library with the same parameters as previous example:

```bash
sacad_r library_directory 600 AlbumArt.jpg
```

## License

[Mozilla Public License Version 2.0](https://www.mozilla.org/MPL/2.0/)
