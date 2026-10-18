# /// script
# requires-python = ">=3.10"
# dependencies = ["pexpect>=4.9,<5", "pyte>=0.8.2,<1", "Pillow>=11,<13"]
# ///
"""Record the real Herdr picker in a disposable session and render MP4 + GIF."""

import argparse
import json
import os
import shutil
import subprocess
import tempfile
import time
import uuid
from pathlib import Path

import pexpect
import pyte
from PIL import Image, ImageDraw, ImageFont

PROJECT = Path(__file__).resolve().parents[1]
PLUGIN_ID = "herdr-workspaces-manager"
COLS, ROWS, FPS = 130, 36, 5
CELL_W, CELL_H = 11, 23
MARGIN_X, MARGIN_Y = 34, 38
WIDTH = COLS * CELL_W + 2 * MARGIN_X
HEIGHT = ROWS * CELL_H + 2 * MARGIN_Y
COLORS = {
    "black": "#151b24",
    "red": "#f27572",
    "green": "#98c379",
    "yellow": "#e5c07b",
    "blue": "#61afef",
    "magenta": "#c678dd",
    "cyan": "#56b6c2",
    "white": "#dce4ee",
    "default": "#dce4ee",
}
ANSI16 = (
    "#151b24",
    "#d66969",
    "#98c379",
    "#e5c07b",
    "#61afef",
    "#c678dd",
    "#56b6c2",
    "#dce4ee",
    "#555e6b",
    "#f27572",
    "#a9d78e",
    "#ffd391",
    "#80bdff",
    "#d79bea",
    "#76d5df",
    "#ffffff",
)


class RecordingScreen(pyte.Screen):
    """Ignore terminal status requests that pyte cannot answer in a headless PTY."""

    def report_device_status(self, *args, **kwargs):
        pass


def font_from(paths, size):
    for path in paths:
        if path.is_file():
            return ImageFont.truetype(str(path), size)
    raise RuntimeError(
        f"No suitable font found; install one of: {', '.join(map(str, paths))}"
    )


def fonts():
    home = Path.home()
    regular = font_from(
        [
            home / "Library/Fonts/JetBrainsMono-Regular.ttf",
            Path("/System/Library/Fonts/Menlo.ttc"),
            Path("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"),
        ],
        17,
    )
    bold = font_from(
        [
            home / "Library/Fonts/JetBrainsMonoNL-Bold.ttf",
            Path("/System/Library/Fonts/Menlo.ttc"),
            Path("/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf"),
        ],
        17,
    )
    # JetBrains Mono lacks ★; a symbol-capable fallback also renders ⌂ clearly.
    symbols = font_from(
        [
            Path("/System/Library/Fonts/Apple Symbols.ttf"),
            Path("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
        ],
        20,
    )
    return regular, bold, symbols


def color(value, default):
    if value in (None, "default"):
        return default
    if value in COLORS:
        return COLORS[value]
    if isinstance(value, str) and len(value) == 6:
        return "#" + value
    try:
        number = int(value)
        if number < 16:
            return ANSI16[number]
        if number < 232:
            index = number - 16
            levels = (0, 95, 135, 175, 215, 255)
            return (
                f"#{levels[index // 36]:02x}{levels[(index // 6) % 6]:02x}"
                f"{levels[index % 6]:02x}"
            )
        gray = 8 + 10 * (number - 232)
        return f"#{gray:02x}{gray:02x}{gray:02x}"
    except (TypeError, ValueError):
        return default


def render(screen, regular, bold, symbols):
    image = Image.new("RGB", (WIDTH, HEIGHT), "#111820")
    draw = ImageDraw.Draw(image)
    draw.rounded_rectangle(
        (13, 13, WIDTH - 14, HEIGHT - 14),
        17,
        fill="#1b232d",
        outline="#37424f",
        width=2,
    )
    draw.rounded_rectangle(
        (MARGIN_X - 11, MARGIN_Y - 10, WIDTH - MARGIN_X + 11, HEIGHT - MARGIN_Y + 10),
        6,
        fill="#111820",
    )
    for y in range(ROWS):
        runs = []
        for x in range(COLS):
            character = screen.buffer[y][x]
            foreground = color(character.fg, "#dce4ee")
            background = color(character.bg, "#111820")
            if character.reverse:
                foreground, background = background, foreground
            style = (foreground, background, character.bold)
            if runs and runs[-1][1] == style:
                runs[-1][2] += character.data
            else:
                runs.append([x, style, character.data])
        for x, (foreground, background, is_bold), text in runs:
            px, py = MARGIN_X + x * CELL_W, MARGIN_Y + y * CELL_H
            if background != "#111820":
                draw.rectangle(
                    (px, py, px + len(text) * CELL_W, py + CELL_H), fill=background
                )
            if text.strip():
                plain = text.translate(str.maketrans({"★": " ", "⌂": " "}))
                draw.text(
                    (px, py), plain, font=bold if is_bold else regular, fill=foreground
                )
                for index, glyph in enumerate(text):
                    if glyph in ("★", "⌂"):
                        draw.text(
                            (px + index * CELL_W, py - 2),
                            glyph,
                            font=symbols,
                            fill=foreground,
                        )
    return image.tobytes()


def create_demo_home(root):
    projects = {
        "Work/Acme/atlas-api": ("src", "tests"),
        "Work/Acme/atlas-web": ("src", "public"),
        "Work/Orion/analytics-service": ("src", "docs"),
        "Work/Orion/design-system": ("components", "docs"),
        "Personal/garden-notes": ("notes", "photos"),
        "Personal/finance-dashboard": ("src", "data"),
        "Personal/cli-tools": ("scripts",),
        "Labs/agent-playground": ("experiments",),
        "Labs/terminal-ui": ("src", "examples"),
    }
    for name, folders in projects.items():
        project = root / name
        for folder in folders:
            (project / folder).mkdir(parents=True, exist_ok=True)
        (project / "README.md").write_text(f"# {project.name}\n")

    config = root / ".config/herdr/plugins/config" / PLUGIN_ID
    config.mkdir(parents=True)
    (config / "config.json").write_text(
        json.dumps(
            {
                "roots": [str(root / name) for name in ("Work", "Personal", "Labs")],
                "favorites": [
                    str(root / "Work/Acme/atlas-api"),
                    str(root / "Personal/garden-notes"),
                ],
                "max_depth": 8,
                "focus_created": False,
            },
            indent=2,
        )
    )
    (root / ".zshrc").write_text("PROMPT='demo@herdr > '\n")
    (root / ".bashrc").write_text("PS1='demo@herdr > '\n")


def herdr_env(root):
    env = {
        key: value for key, value in os.environ.items() if not key.startswith("HERDR_")
    }
    env.update(
        HOME=str(root),
        HERDR_CONFIG_PATH=str(root / ".config/herdr/config.toml"),
        TERM="xterm-256color",
        COLORTERM="truecolor",
    )
    return env


def cli(env, *args):
    completed = subprocess.run(
        ["herdr", *args], env=env, text=True, capture_output=True, check=False
    )
    if completed.returncode:
        raise RuntimeError(f"herdr {' '.join(args)} failed: {completed.stderr.strip()}")
    return completed.stdout


def record(root, env, session, output):
    regular, bold, symbols = fonts()
    cli(env, "--session", session, "plugin", "link", str(PROJECT))
    child = pexpect.spawn(
        "herdr",
        ["--session", session],
        env=env,
        cwd=str(root),
        dimensions=(ROWS, COLS),
        encoding=None,
    )
    screen = RecordingScreen(COLS, ROWS)
    stream = pyte.ByteStream(screen)
    encoder = None
    started = time.monotonic()
    frame_number = 0

    def pump(seconds):
        nonlocal frame_number
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            try:
                stream.feed(child.read_nonblocking(size=65536, timeout=0.025))
            except pexpect.TIMEOUT:
                pass
            except pexpect.EOF as error:
                raise RuntimeError(
                    "Herdr exited before the recording finished"
                ) from error
            if encoder is not None:
                elapsed = time.monotonic() - started
                while frame_number / FPS <= elapsed:
                    encoder.stdin.write(render(screen, regular, bold, symbols))
                    frame_number += 1

    try:
        pump(1.5)
        child.send(b"\r")  # dismiss first-run welcome
        pump(0.7)
        child.send(b"\x1b")  # close first-run settings
        pump(0.7)
        if not any("demo@herdr" in line for line in screen.display):
            raise RuntimeError(
                "Neutral shell prompt not visible; refusing to record a username"
            )

        encoder = subprocess.Popen(
            [
                "ffmpeg",
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "rawvideo",
                "-pixel_format",
                "rgb24",
                "-video_size",
                f"{WIDTH}x{HEIGHT}",
                "-framerate",
                str(FPS),
                "-i",
                "-",
                "-c:v",
                "libx264",
                "-preset",
                "medium",
                "-crf",
                "21",
                "-pix_fmt",
                "yuv420p",
                "-movflags",
                "+faststart",
                str(output),
            ],
            stdin=subprocess.PIPE,
        )
        started = time.monotonic()
        cli(
            env,
            "--session",
            session,
            "plugin",
            "pane",
            "open",
            "--plugin",
            PLUGIN_ID,
            "--entrypoint",
            "picker",
        )
        pump(2.8)

        # Real key input drives the real picker; pauses make each state readable.
        steps = [
            (b"atlas-web", 1.5),
            (b"\x06", 1.4),
            (b"\x15", 0.8),
            (b"\x07", 2.6),  # Favorites
            (b"\x07", 1.5),  # Recent (initially empty)
            (b"\x07", 2.6),  # Work
            (b"atlas", 1.5),
            (b"\x15", 0.7),
            (b"\x07", 2.5),  # Personal
            (b"\x07", 2.5),  # Labs
            (b"\x07", 2.0),  # All
            (b"\x07", 1.8),  # Favorites again
            (b"\x14", 1.2),
            (b"\x1b[B", 0.7),
            (b"\x14", 1.3),
            (b"\r", 2.8),
            (b"\r", 2.7),
        ]
        for keys, seconds in steps:
            child.send(keys)
            pump(seconds)
        pump(0.8)

        workspaces = json.loads(cli(env, "--session", session, "workspace", "list"))[
            "result"
        ]["workspaces"]
        labels = {workspace["label"] for workspace in workspaces}
        if not {"garden-notes", "atlas-api"}.issubset(labels):
            raise RuntimeError(
                f"Expected workspaces were not created: {sorted(labels)}"
            )
        # Reopen the picker to show the newly populated Recent scope.
        cli(
            env,
            "--session",
            session,
            "plugin",
            "pane",
            "open",
            "--plugin",
            PLUGIN_ID,
            "--entrypoint",
            "picker",
        )
        pump(1.4)
        child.send(b"\x07")  # Favorites
        pump(0.7)
        child.send(b"\x07")  # Recent
        pump(2.5)
        child.send(b"\x1b")
        pump(0.5)
        encoder.stdin.close()
        if encoder.wait() != 0:
            raise RuntimeError("ffmpeg failed to encode the MP4")
        encoder = None
        return frame_number / FPS
    finally:
        if encoder is not None:
            encoder.stdin.close()
            encoder.wait()
        child.close(force=True)
        subprocess.run(
            ["herdr", "session", "stop", session],
            env=env,
            capture_output=True,
            check=False,
        )
        subprocess.run(
            ["herdr", "session", "delete", session],
            env=env,
            capture_output=True,
            check=False,
        )


def make_gif(mp4, gif):
    filtergraph = (
        "fps=5,scale=1200:-1:flags=lanczos,split[a][b];"
        "[a]palettegen=max_colors=64:stats_mode=diff[p];"
        "[b][p]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle"
    )
    subprocess.run(
        [
            "ffmpeg",
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-ss",
            "1",
            "-i",
            str(mp4),
            "-filter_complex",
            filtergraph,
            "-loop",
            "0",
            str(gif),
        ],
        check=True,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=PROJECT / "docs",
        help="Destination for the MP4 and GIF (default: docs/)",
    )
    args = parser.parse_args()
    if os.environ.get("HERDR_ENV") != "1":
        parser.error("run this from a Herdr-managed pane (HERDR_ENV=1)")
    if not (PROJECT / "target/release/herdr-workspaces-manager").is_file():
        parser.error("build the current plugin first: task build")
    for binary in ("herdr", "ffmpeg"):
        if shutil.which(binary) is None:
            parser.error(f"{binary} is required in PATH")

    destination = args.output_dir.resolve()
    destination.mkdir(parents=True, exist_ok=True)
    # Herdr's Unix socket path has a strict length limit; keep both names short.
    with tempfile.TemporaryDirectory(prefix="hd-", dir="/tmp") as directory:
        root = Path(directory)
        create_demo_home(root)
        env = herdr_env(root)
        session = "vid-" + uuid.uuid4().hex[:8]
        mp4 = destination / f".workspaces-manager-demo-{session}.mp4"
        gif = destination / f".workspaces-manager-demo-{session}.gif"
        try:
            duration = record(root, env, session, mp4)
            make_gif(mp4, gif)
            mp4.replace(destination / "workspaces-manager-demo.mp4")
            gif.replace(destination / "workspaces-manager-demo.gif")
        finally:
            mp4.unlink(missing_ok=True)
            gif.unlink(missing_ok=True)
    print(f"Recorded {duration:.1f}s in {destination}")


if __name__ == "__main__":
    main()
