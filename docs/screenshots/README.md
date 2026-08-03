# Screenshots

Empty on purpose: capturing these needs a running panel and a real tablet on the same network,
which nothing in CI can do. Four images belong here, and the main `README.md` links to this file
until they exist.

Save them as PNG at the names below, then replace the *Screenshots* section of the root
`README.md` with the image tags.

| File | What it should show |
| --- | --- |
| `deck.png` | The tablet running a full grid, landscape, connected — the status chip showing a host name and a round-trip time in milliseconds. This is the one that sells the project; it should be a profile somebody would actually use, not the 15-button default. |
| `editor.png` | The panel's layout editor with the per-button dialog open on a real shortcut, so the key-capture field and the icon picker are both visible. |
| `pairing.png` | The pair screen with the QR code and the six-digit code, mid-countdown. Crop or replace the code — it is single-use and expires, but a screenshot of a live pairing window is still a pairing window. |
| `dashboard.png` | The panel's status screen with a device connected, live CPU and memory, and the log tail showing real output. |

Two things to check before committing any of them:

- **No real fingerprints, tokens or device IDs.** The dashboard and the devices table both show
  identifiers that are specific to a machine. They are not secrets — the admin token is, and it is
  never on screen — but there is no reason to publish them either.
- **Dark theme.** The panel follows the system theme and the deck is dark-only, so a light-theme
  panel screenshot next to a dark deck one looks like two different projects.
