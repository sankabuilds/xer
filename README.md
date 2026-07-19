# 🌴Xer

`xer` is a Rust command-line tool for downloading media from X (Twitter) bookmarks.

## Installation

1. Pre-built binaries are available on the release page.
2. Install directly from GitHub with Cargo:

```bash
cargo install --git https://github.com/sankabuilds/xer
```

## Usage

`xer` uses a cookie file for X authentication and currently supports the `x bookmarks` subcommand.

Basic command structure:

```bash
xer --cookie <cookie-file> x bookmarks [--limit <n>] [--all]
```

Options:

- `--cookie <cookie-file>`: path to the cookie file used for authentication with X.
- `x bookmarks --limit <n>`: download up to `n` bookmarks.
- `x bookmarks --all`: download all available bookmarks.

The default limit is `100`, so `xer --cookie cookies.json x bookmarks --limit 100` and `xer --cookie cookies.json x bookmarks` have the same effect.

Example:

```bash
xer --cookie cookies.json x bookmarks --limit 100
```

To download all bookmarks:

```bash
xer --cookie cookies.json x bookmarks --all
```

## Cookie file

- The cookie file should contain valid JSON values.
- Export cookies from your X account using the Cookie Editor extension:
  https://chromewebstore.google.com/detail/cookie-editor/hlkenndednhfkekhgcdicdfddnkalmdm

## Notes

- The `--cookie` option is required for site access.
- `xer` currently includes a single top-level command group: `x`, with a `bookmarks` subcommand.
