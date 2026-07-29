# xer

_Xer_ is a simple CLI tool that you can use to download bookmarks from your social media accounts.

### Currently Supported Sites:

| Site        | Status |
| ----------- | ------ |
| X (Twitter) | ✅     |
| Instagram   | ✅     |

### Usage

```
Xer - Download social media bookmarks

Usage: xer.exe [OPTIONS] <COMMAND>

Commands:
  x     𝕏 - Download X/Twitter media
  gram  Instagram - Download Instagram media
  help  Print this message or the help of the given subcommand(s)

Options:
  -c, --cookie <COOKIE>  Path to the cookie file
  -v, --verbose          Verbose mode
  -h, --help             Print help
  -V, --version          Print version
```

### Examples

To download the latest ~100 photos & videos from your X bookmarks.

```
xer --cookie <path to the cookie file> x bookmarks
```

To download the latest ~500 photos & videos from your X bookmarks.

```
xer --cookie <path to the cookie file> x bookmarks --limit 500
```

To download all the available photos & videos from your X bookmarks.

```
xer --cookie <path to the cookie file> x bookmarks --all
```

_Instagram works the same way with similar options:_

To download the latest ~100 photos & videos from your Instagram bookmarks.

```
xer --cookie <path to the cookie file> gram bookmarks
```

### Cookie File

The cookie file should contain valid JSON. You can export your account's cookies using the Cookie-Editor chrome extension:

https://chromewebstore.google.com/detail/cookie-editor/hlkenndednhfkekhgcdicdfddnkalmdm
