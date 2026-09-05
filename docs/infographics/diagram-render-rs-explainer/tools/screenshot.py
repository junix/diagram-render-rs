#!/usr/bin/env python3
"""Slice-based deterministic screenshots of index.html via chrome-headless-shell.

Recipe (fleet-stable on this machine): fixed chrome-headless-shell binary,
no --headless=new, --disable-gpu --force-color-profile=srgb, viewport
1200 CSS px at deviceScaleFactor 2. Each slice is scrolled into place with
scrollY read back and asserted before capture. Slices are stitched with
magick, whose -strip pass removes PNG auxiliary chunks.

Outputs renders/full@2x.png (2400 == 1200x2 wide, page CSS height x2 tall),
renders/grayscale.png, renders/thumb.png.

Usage:
    python3 tools/screenshot.py [--tree .] [--port 9377] [--slice-h 800]
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import shutil
import socket
import struct
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import DEFAULT_CHROME, DEFAULT_MAGICK, GateError, resolve_tree  # noqa: E402

SLICE_DEFAULT = 800


class CDP:
    """Minimal Chrome DevTools Protocol client over a raw localhost socket.

    WebSocket text frames only; client frames masked per RFC 6455. Handles
    ping frames by replying with a pong.
    """

    def __init__(self, host: str, port: int, path: str):
        self.sock = socket.create_connection((host, port), timeout=60)
        self.sock.settimeout(60)
        key = base64.b64encode(os.urandom(16)).decode()
        request = (
            f"GET {path} HTTP/1.1\r\n"
            f"Host: {host}:{port}\r\n"
            "Upgrade: websocket\r\n"
            "Connection: Upgrade\r\n"
            f"Sec-WebSocket-Key: {key}\r\n"
            "Sec-WebSocket-Version: 13\r\n\r\n"
        )
        self.sock.sendall(request.encode())
        response = b""
        while b"\r\n\r\n" not in response:
            chunk = self.sock.recv(4096)
            if not chunk:
                raise GateError("websocket handshake failed")
            response += chunk
        if b" 101 " not in response.split(b"\r\n", 1)[0]:
            raise GateError(f"websocket upgrade refused: {response[:200]!r}")
        self._id = 0
        self._buf = b""

    def _read_exact(self, n: int) -> bytes:
        while len(self._buf) < n:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise GateError("socket closed mid-frame")
            self._buf += chunk
        out, self._buf = self._buf[:n], self._buf[n:]
        return out

    def _recv_frame(self) -> tuple[int, bytes]:
        head = self._read_exact(2)
        opcode = head[0] & 0x0F
        masked = bool(head[1] & 0x80)
        length = head[1] & 0x7F
        if length == 126:
            (length,) = struct.unpack(">H", self._read_exact(2))
        elif length == 127:
            (length,) = struct.unpack(">Q", self._read_exact(8))
        if masked:
            mask = self._read_exact(4)
            data = bytearray(self._read_exact(length))
            for i in range(length):
                data[i] ^= mask[i % 4]
            payload = bytes(data)
        else:
            payload = self._read_exact(length)
        return opcode, payload

    def _send_frame(self, opcode: int, payload: bytes) -> None:
        mask = os.urandom(4)
        header = bytearray([0x80 | opcode])
        n = len(payload)
        if n < 126:
            header.append(0x80 | n)
        elif n < 65536:
            header.append(0x80 | 126)
            header += struct.pack(">H", n)
        else:
            header.append(0x80 | 127)
            header += struct.pack(">Q", n)
        masked = bytearray(payload[i] ^ mask[i % 4] for i in range(n))
        self.sock.sendall(bytes(header) + mask + masked)

    def call(self, method: str, params: dict | None = None) -> dict:
        self._id += 1
        message_id = self._id
        frame = json.dumps({"id": message_id, "method": method, "params": params or {}})
        self._send_frame(1, frame.encode())
        while True:
            opcode, payload = self._recv_frame()
            if opcode == 9:  # ping -> pong
                self._send_frame(10, payload)
                continue
            if opcode != 1:
                continue
            message = json.loads(payload.decode())
            if message.get("id") != message_id:
                continue
            if "error" in message:
                raise GateError(f"CDP {method} failed: {message['error']}")
            return message.get("result", {})

    def close(self) -> None:
        try:
            self._send_frame(8, b"")
        except OSError:
            pass
        self.sock.close()


def evaluate(cdp: CDP, expression: str):
    result = cdp.call(
        "Runtime.evaluate",
        {"expression": expression, "returnByValue": True},
    )
    if "exceptionDetails" in result:
        raise GateError(f"evaluation failed: {expression}")
    return result["result"].get("value")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tree", default=None)
    parser.add_argument("--port", type=int, default=9377)
    parser.add_argument("--slice-h", type=int, default=SLICE_DEFAULT)
    args = parser.parse_args()

    tree = resolve_tree(args.tree)
    chrome = Path(os.environ.get("DRR_CHROME", DEFAULT_CHROME)).expanduser()
    magick = os.environ.get("DRR_MAGICK", DEFAULT_MAGICK)
    page = (tree / "index.html").resolve()
    if not page.is_file():
        raise GateError("index.html not found; run the page tool first")

    shots = Path("/tmp/ign-drr/shots")
    shutil.rmtree(shots, ignore_errors=True)
    shots.mkdir(parents=True)
    profile = Path("/tmp/ign-drr/chrome-profile")
    shutil.rmtree(profile, ignore_errors=True)

    chrome_path = str(chrome)
    if not Path(chrome_path).is_file():
        raise GateError(f"chrome-headless-shell not found: {chrome_path}")

    flags = [
        chrome_path,
        f"--remote-debugging-port={args.port}",
        "--disable-gpu",
        "--force-color-profile=srgb",
        "--hide-scrollbars",
        "--no-first-run",
        "--no-default-browser-check",
        f"--user-data-dir={profile}",
        "about:blank",
    ]
    proc = subprocess.Popen(flags, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        ws_url = None
        for _ in range(100):
            time.sleep(0.15)
            try:
                listing = json.loads(
                    urllib.request.urlopen(
                        f"http://127.0.0.1:{args.port}/json", timeout=2
                    ).read()
                )
                for target in listing:
                    if target.get("type") == "page":
                        ws_url = target["webSocketDebuggerUrl"]
                        break
                if ws_url:
                    break
            except (OSError, ValueError):
                continue
        if not ws_url:
            raise GateError("could not reach the DevTools endpoint")

        from urllib.parse import urlsplit

        parts = urlsplit(ws_url)
        host = parts.hostname or "127.0.0.1"
        port = parts.port or args.port
        path = parts.path or "/"
        cdp = CDP(host, port, path)
        try:
            cdp.call("Page.enable")
            cdp.call(
                "Emulation.setDeviceMetricsOverride",
                {"width": 1200, "height": args.slice_h, "deviceScaleFactor": 2, "mobile": False},
            )
            cdp.call("Page.navigate", {"url": page.as_uri()})
            deadline = time.time() + 30
            while time.time() < deadline:
                state = evaluate(
                    cdp,
                    "(function(){return document.readyState + '|'"
                    " + (document.fonts ? document.fonts.status : 'na')})()",
                )
                if state == "complete|loaded":
                    break
                time.sleep(0.1)
            else:
                raise GateError("page never reached readyState complete with fonts loaded")

            page_height = evaluate(
                cdp,
                "Math.max(document.documentElement.scrollHeight,"
                " document.body.scrollHeight)",
            )
            page_width = evaluate(
                cdp,
                "Math.max(document.documentElement.scrollWidth,"
                " document.body.scrollWidth)",
            )
            if not isinstance(page_height, (int, float)) or page_height < 500:
                raise GateError(f"implausible page height: {page_height}")
            if page_width > 1200:
                raise GateError(f"horizontal overflow: scrollWidth {page_width} > 1200")

            slices: list[Path] = []
            offset = 0
            index = 0
            while offset < page_height:
                height = min(args.slice_h, int(page_height) - offset)
                cdp.call(
                    "Emulation.setDeviceMetricsOverride",
                    {
                        "width": 1200,
                        "height": height,
                        "deviceScaleFactor": 2,
                        "mobile": False,
                    },
                )
                scrolled = evaluate(
                    cdp,
                    "(function(){window.scrollTo(0, " + str(offset) + ");"
                    "return window.scrollY;})()",
                )
                if scrolled != offset:
                    raise GateError(
                        f"scroll assertion failed at slice {index}: "
                        f"scrollY {scrolled} != {offset}"
                    )
                shot = cdp.call("Page.captureScreenshot", {"format": "png"})
                data = base64.b64decode(shot["data"])
                slice_path = shots / f"slice-{index:03d}.png"
                slice_path.write_bytes(data)
                slices.append(slice_path)
                offset += height
                index += 1

            expected_w = 2400
            expected_h = int(page_height) * 2
            stitched = shots / "stitched.png"
            subprocess.run(
                [magick, *[str(s) for s in slices], "-append", str(stitched)],
                check=True,
            )
            identify = subprocess.run(
                [magick, "identify", "-format", "%w %h", str(stitched)],
                check=True, capture_output=True, text=True,
            ).stdout.split()
            got_w, got_h = int(identify[0]), int(identify[1])
            if got_w != expected_w:
                raise GateError(f"full width {got_w} != {expected_w}")
            if got_h != expected_h:
                raise GateError(f"full height {got_h} != {expected_h}")

            renders = tree / "renders"
            renders.mkdir(exist_ok=True)
            # png:exclude-chunk=time: ImageMagick's PNG writer emits a tIME
            # timestamp chunk even under -strip, which breaks byte stability.
            no_time = ["-define", "png:exclude-chunk=time"]
            subprocess.run(
                [magick, str(stitched), "-strip", *no_time,
                 str(renders / "full@2x.png")], check=True
            )
            subprocess.run(
                [magick, str(renders / "full@2x.png"), "-grayscale", "Rec709Luminance",
                 "-strip", *no_time, str(renders / "grayscale.png")],
                check=True,
            )
            subprocess.run(
                [magick, str(renders / "full@2x.png"), "-resize", "480x", "-strip",
                 *no_time, str(renders / "thumb.png")],
                check=True,
            )
            print(
                f"screenshots: page {int(page_width)}x{int(page_height)} CSS px, "
                f"{len(slices)} slices, full@2x {got_w}x{got_h}"
            )
        finally:
            cdp.close()
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateError as error:
        print(f"SCREENSHOT FAILED: {error}", file=sys.stderr)
        raise SystemExit(1)
