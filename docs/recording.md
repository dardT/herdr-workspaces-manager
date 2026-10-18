# Recording the demo

The README animation and its full-length video are generated from a **real run** of the plugin, not from a mock UI. `scripts/record_demo.py` creates a disposable project tree and Herdr session, sends real keystrokes to the picker, records the terminal's output, and renders it to:

- `docs/workspaces-manager-demo.mp4` — full H.264 recording, linked from the README.
- `docs/workspaces-manager-demo.gif` — smaller, autoplaying inline preview for GitHub Markdown.

GitHub READMEs do not reliably embed repository MP4s as playable videos. The README therefore embeds the GIF as an image and links it to the MP4.

## Prerequisites

Run this **inside a Herdr-managed pane** (`HERDR_ENV=1`) on a machine with:

- Herdr 0.9.3+ and the Rust toolchain needed to build this checkout.
- [Task](https://taskfile.dev/), [uv](https://docs.astral.sh/uv/), and FFmpeg (`ffmpeg` in `PATH`). Manage these command-line tools with mise if that is how your machine is set up.
- A usable monospace font: JetBrains Mono or Menlo on macOS, or DejaVu Sans Mono on Linux. The renderer also needs Apple Symbols (macOS) or DejaVu Sans (Linux) for **★** and **⌂**.
- A zsh or bash shell for the disposable session's neutral `demo@herdr` prompt.

Python packages are declared in the script's [PEP 723 inline metadata](https://packaging.python.org/en/latest/specifications/inline-script-metadata/): **pexpect** drives an actual pseudo-terminal, **pyte** reconstructs its ANSI screen, and **Pillow** draws that screen with the chosen fonts. `uv run --script` installs these into its cache automatically; they are **not plugin runtime dependencies**. FFmpeg encodes the rendered frames and produces the GIF.

## Regenerate the assets

From this repository, run:

```sh
task demo:record
```

The task runs `cargo build --release --locked` first, then `uv run --script scripts/record_demo.py`. It replaces the MP4 and GIF only after both have encoded successfully. To inspect outputs without changing the checked-in assets, run `uv run --script scripts/record_demo.py --output-dir /tmp/herdr-demo-output` after `task build`.

Check the generated media with `ffprobe -v error -show_entries stream=width,height -show_entries format=duration,size -of default=nw=1 docs/workspaces-manager-demo.mp4` and inspect both the opening and later frames. In particular, confirm that the prompt says `demo@herdr`, the **★/⌂** glyphs render, the scope changes are visible, and the GIF begins with a populated picker.

## How the recording works

1. **Prepare isolated data.** The script creates a temporary home with believable `Work/`, `Personal/`, and `Labs/` projects, nested source folders, three configured roots, and two favorites. Its plugin configuration and shell startup files live inside that temporary home; the real user's plugin configuration is not edited.
2. **Launch a separate Herdr session.** A unique named session uses that temporary `HOME` and `HERDR_CONFIG_PATH`. The plugin is linked only into the disposable session. The script checks for the neutral prompt before recording, then opens the real popup through `herdr plugin pane open`.
3. **Capture real terminal output.** `pexpect` gives Herdr a 130×36-cell PTY and sends typed search text plus control-key bytes. `pyte.ByteStream` consumes the output and maintains the screen state. The scripted pauses let asynchronous scans finish and keep scopes legible; a final workspace-list check confirms the batch actually created two workspaces.
4. **Draw and encode frames.** Pillow renders each terminal cell at 5 frames per second. A symbol-capable fallback font draws ★ and ⌂, since the primary JetBrains Mono font lacks ★. Frames stream directly into FFmpeg's H.264 encoder instead of being held in memory. FFmpeg then makes a 1200-pixel-wide, 64-color GIF with a generated palette; the GIF skips the first second of popup startup.
5. **Clean up.** The script stops and deletes **only its own named session**, removes the temporary home, and leaves the regular Herdr session alone. If the process is killed abruptly, inspect any leftover `/tmp/hd-*` demo home before removing it. To list its `vid-` sessions, use that home's `HOME` and `HERDR_CONFIG_PATH`; the regular user's session list will not include them.

The sequence currently demonstrates search, a new favorite, All/Favorites/Recent/Work/Personal/Labs scopes, batch review, workspace creation, and the populated Recent scope after reopening. To change the story, edit the `steps` list in `record()`; each entry is a byte sequence and a pause in seconds. For example, `b"\x06"` is Ctrl+F, `b"\x07"` is Ctrl+G, `b"\x14"` is Ctrl+T, and `b"\r"` is Enter. Keep pauses after scope switches because directory discovery is asynchronous. The script's constants control terminal size, frame rate, and font cell dimensions; `make_gif()` controls preview scale and palette size.

**Privacy:** Do not replace the disposable project tree with your own home directory without reviewing every recorded frame and path. The script rejects a session whose neutral prompt did not appear, but it cannot detect all other sensitive data a customized terminal might show.
