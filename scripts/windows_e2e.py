"""Windows end-to-end check for Saykey: real global shortcuts, microphone, and insertion.

Runs saykey.exe against a local mock provider and types only into windows this
script creates (a Tk test window and a Notepad file in a temporary directory).
It synthesizes keyboard input, so do not type while it runs. The clipboard's
text is saved and restored.

    python scripts/windows_e2e.py [--exe target/release/saykey.exe] [--play speech.wav]
    cargo build --release --locked --features test-audio --target-dir target/e2e
    python scripts/windows_e2e.py --exe target/e2e/release/saykey.exe --audio speech.wav

With a real microphone, --play plays a WAV through the speakers during each
recording so the microphone hears something. Without one, --audio replays a
16-bit WAV in place of the microphone; this needs a build with the test-audio
feature and exercises everything except device capture. --live additionally
dictates through the real provider configured by OPENAI_API_KEY and prints the
result (paid request).
"""

import argparse
import ctypes
import ctypes.wintypes as wt
import json
import os
import queue
import subprocess
import sys
import tempfile
import threading
import time
import tkinter as tk
import winsound
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

user32 = ctypes.WinDLL("user32", use_last_error=True)
VK_CONTROL, VK_MENU, VK_SPACE, VK_ESCAPE, VK_V, VK_S = 0x11, 0x12, 0x20, 0x1B, 0x56, 0x53
KEYEVENTF_KEYUP = 2
WM_CLOSE = 0x10


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [("wVk", wt.WORD), ("wScan", wt.WORD), ("dwFlags", wt.DWORD),
                ("time", wt.DWORD), ("dwExtraInfo", ctypes.c_size_t)]


class MOUSEINPUT(ctypes.Structure):
    _fields_ = [("dx", wt.LONG), ("dy", wt.LONG), ("mouseData", wt.DWORD),
                ("dwFlags", wt.DWORD), ("time", wt.DWORD), ("dwExtraInfo", ctypes.c_size_t)]


class INPUT(ctypes.Structure):
    class _U(ctypes.Union):
        _fields_ = [("ki", KEYBDINPUT), ("mi", MOUSEINPUT)]

    _fields_ = [("type", wt.DWORD), ("u", _U)]


def key(vk, up=False):
    event = INPUT(type=1)
    event.u.ki = KEYBDINPUT(vk, 0, KEYEVENTF_KEYUP if up else 0, 0, 0)
    assert user32.SendInput(1, ctypes.byref(event), ctypes.sizeof(INPUT)) == 1


def chord(*vks, hold=0.05):
    for vk in vks:
        key(vk)
    time.sleep(hold)
    for vk in reversed(vks):
        key(vk, up=True)
    time.sleep(0.1)


def toggle():
    chord(VK_CONTROL, VK_MENU, VK_SPACE)


def focus(hwnd):
    # Windows only lets the process that sent the last input change the foreground.
    # The Ctrl tap keeps the Alt release from opening the target's window menu.
    key(VK_MENU)
    user32.SetForegroundWindow(hwnd)
    key(VK_CONTROL)
    key(VK_CONTROL, up=True)
    key(VK_MENU, up=True)
    time.sleep(0.3)
    return user32.GetForegroundWindow() == hwnd


class Provider:
    """OpenAI-compatible mock that records requests and returns scripted text."""

    def __init__(self):
        self.text, self.status, self.delay, self.requests = "", 200, 0.0, []
        provider = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_POST(self):
                body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
                provider.requests.append((self.path, body))
                time.sleep(provider.delay)
                if provider.status != 200:
                    self.send_response(provider.status)
                    self.end_headers()
                    return
                if self.path.endswith("/audio/transcriptions"):
                    payload = {"text": provider.text}
                else:
                    payload = {"choices": [{"message": {"content": provider.text}}]}
                data = json.dumps(payload).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        self.url = f"http://127.0.0.1:{self.server.server_port}/v1"


def overlay_visible():
    hwnd = user32.FindWindowW("SaykeyOverlay", None)
    return bool(hwnd) and bool(user32.IsWindowVisible(hwnd))


class Saykey:
    def __init__(self, exe, config, audio=None):
        env = dict(os.environ)
        if audio:
            env["SAYKEY_TEST_AUDIO"] = str(Path(audio).resolve())
        self.proc = subprocess.Popen(
            [exe, "--config", str(config), "run"], env=env,
            stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
            text=True, encoding="utf-8", errors="replace",
        )
        self.lines, self.cv = [], threading.Condition()
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        for line in self.proc.stderr:
            with self.cv:
                self.lines.append(line.rstrip())
                self.cv.notify_all()

    def mark(self):
        with self.cv:
            return len(self.lines)

    def wait_for(self, *needles, since=0, timeout=15):
        deadline = time.time() + timeout
        with self.cv:
            while True:
                for line in self.lines[since:]:
                    if any(needle in line for needle in needles):
                        return line
                remaining = deadline - time.time()
                if remaining <= 0 or self.proc.poll() is not None:
                    raise AssertionError(f"Timed out waiting for {needles}; log: {self.lines[since:]}")
                self.cv.wait(remaining)

    def stop(self):
        self.proc.kill()
        self.proc.wait()


class Ui:
    """Tk owns the main thread; scenarios run on a worker and call into Tk via a queue."""

    def __init__(self):
        self.root = tk.Tk()
        self.root.title("Saykey E2E target")
        self.root.geometry("640x240+80+80")
        self.text = tk.Text(self.root, font=("Segoe UI", 12))
        self.text.pack(fill="both", expand=True)
        self.other = tk.Toplevel(self.root)
        self.other.title("Saykey E2E other field")
        self.other.geometry("400x120+760+80")
        self.other_text = tk.Text(self.other)
        self.other_text.pack(fill="both", expand=True)
        self.calls = queue.Queue()
        self.root.after(20, self._pump)

    def _pump(self):
        while not self.calls.empty():
            fn, result = self.calls.get()
            try:
                result.put((True, fn()))
            except Exception as error:  # noqa: BLE001 - forwarded to the worker
                result.put((False, error))
        self.root.after(20, self._pump)

    def call(self, fn):
        result = queue.Queue()
        self.calls.put((fn, result))
        ok, value = result.get(timeout=10)
        if not ok:
            raise value
        return value

    def hwnd(self, window):
        return self.call(lambda: int(window.wm_frame(), 16))

    def clear(self):
        self.call(lambda: (self.text.delete("1.0", "end"), self.other_text.delete("1.0", "end")))

    def read(self, other=False):
        widget = self.other_text if other else self.text
        return self.call(lambda: widget.get("1.0", "end-1c"))

    def focus_main(self):
        assert focus(self.hwnd(self.root)), "Could not focus the test window"
        self.call(self.text.focus_force)
        time.sleep(0.2)

    def focus_other(self):
        assert focus(self.hwnd(self.other)), "Could not focus the second test window"
        self.call(self.other_text.focus_force)
        time.sleep(0.2)

    def clipboard(self, value=None):
        def run():
            if value is None:
                try:
                    return self.root.clipboard_get()
                except tk.TclError:
                    return None
            self.root.clipboard_clear()
            self.root.clipboard_append(value)
            self.root.update()
            return value

        return self.call(run)


class Run:
    def __init__(self, args, ui, provider, workdir):
        self.args, self.ui, self.provider, self.workdir = args, ui, provider, workdir
        self.config = workdir / "config.toml"
        self.write_config(self.config)
        self.app = self.launch(self.config)
        self.results = []

    def launch(self, config):
        app = Saykey(self.args.exe, config, self.args.audio)
        app.wait_for("Ready.")
        return app

    def write_config(self, path, **extra):
        values = {
            "base_url": self.provider.url,
            "api_key_env": "",
            "transcription_model": "e2e-model",
            "dictionary": ["Saykey"],
            "max_recording_seconds": 30,
            **extra,
        }
        path.write_text("\n".join(f"{k} = {json.dumps(v)}" for k, v in values.items()), "utf-8")

    def record(self, seconds=1.5):
        since = self.app.mark()
        toggle()
        self.app.wait_for("Recording…", "Microphone error", "Focus a text field", since=since)
        if self.args.play:
            winsound.PlaySound(self.args.play, winsound.SND_FILENAME | winsound.SND_ASYNC)
        time.sleep(seconds)
        toggle()
        return since

    def check(self, name, fn):
        self.provider.text, self.provider.status, self.provider.delay = "", 200, 0.0
        self.ui.clear()
        try:
            fn()
            self.results.append((name, "PASS", ""))
        except Exception as error:  # noqa: BLE001 - report every scenario
            self.results.append((name, "FAIL", str(error)))
        winsound.PlaySound(None, 0)
        time.sleep(0.5)

    # --- scenarios -------------------------------------------------------

    def hold_to_talk(self):
        expected = "Held, released, inserted"
        self.provider.text = expected
        self.ui.focus_main()
        target = self.ui.hwnd(self.ui.root)
        since = self.app.mark()
        for vk in (VK_CONTROL, VK_MENU, VK_SPACE):
            key(vk)
        try:
            self.app.wait_for("Recording…", since=since)
            time.sleep(1.2)
            assert overlay_visible(), "Status overlay not shown while recording"
            assert user32.GetForegroundWindow() == target, "Overlay or Saykey took focus"
            assert not any("Hands-free" in line for line in self.app.lines[since:]), "Hold treated as tap"
        finally:
            for vk in (VK_SPACE, VK_MENU, VK_CONTROL):
                key(vk, up=True)
        line = self.app.wait_for("Inserted", "failed", "retained", since=since)
        assert "Inserted" in line, line
        time.sleep(0.5)
        assert self.ui.read() == expected, f"Field contains {self.ui.read()!r}"
        time.sleep(1.5)
        assert not overlay_visible(), "Overlay still visible after insertion"

    def insert_unicode(self):
        expected = "Grüße aus Zürich – café, naïve ✓ Saykey"
        self.provider.text = expected
        sentinel = f"saykey-e2e-sentinel-{time.time()}"
        self.ui.clipboard(sentinel)
        self.ui.focus_main()
        before = len(self.provider.requests)
        since = self.record()
        line = self.app.wait_for("Inserted", "failed", "discarded", "retained", since=since)
        assert "Inserted" in line, line
        assert any("Hands-free" in line for line in self.app.lines[since:]), "Tap did not keep recording"
        time.sleep(0.5)
        assert self.ui.read() == expected, f"Field contains {self.ui.read()!r}"
        assert len(self.provider.requests) == before + 1, "Expected exactly one upload"
        path, body = self.provider.requests[-1]
        assert path.endswith("/audio/transcriptions"), path
        assert b'name="model"' in body and b"e2e-model" in body and b"Saykey" in body
        assert body.count(b"RIFF") == 1, "Upload is not a WAV file"
        rate = int.from_bytes(body[body.index(b"RIFF") + 24:body.index(b"RIFF") + 28], "little")
        assert rate <= 16_000, f"Uploaded {rate} Hz audio; expected at most 16 kHz"
        assert self.ui.clipboard() == sentinel, "Clipboard was modified"

    def cancel_recording(self):
        self.provider.text = "must not appear"
        self.ui.focus_main()
        before = len(self.provider.requests)
        since = self.app.mark()
        toggle()
        self.app.wait_for("Recording…", since=since)
        time.sleep(0.5)
        chord(VK_CONTROL, VK_MENU, VK_ESCAPE)
        self.app.wait_for("Canceled", since=since)
        time.sleep(1)
        assert len(self.provider.requests) == before, "Canceled recording was uploaded"
        assert self.ui.read() == "", "Canceled recording inserted text"

    def cancel_processing(self):
        self.provider.text, self.provider.delay = "must not appear", 2.0
        self.ui.focus_main()
        since = self.record()
        self.app.wait_for("Transcribing", since=since)
        chord(VK_CONTROL, VK_MENU, VK_ESCAPE)
        self.app.wait_for("Ready.", since=since, timeout=10)
        time.sleep(0.5)
        assert self.ui.read() == "", "Canceled result was inserted"

    def provider_error(self):
        self.provider.status = 500
        self.ui.focus_main()
        since = self.record()
        line = self.app.wait_for("Transcription failed", "Inserted", since=since)
        assert "HTTP 500" in line, line
        assert self.ui.read() == "", "Text inserted despite provider error"

    def focus_change_then_recover(self):
        expected = "Recovered after focus change"
        self.provider.text, self.provider.delay = expected, 2.0
        self.ui.focus_main()
        since = self.record()
        self.app.wait_for("Transcribing", since=since)
        self.ui.focus_other()
        self.app.wait_for("retained", "Inserted", since=since)
        assert self.ui.read() == "" and self.ui.read(other=True) == "", "Inserted into a changed target"
        self.ui.focus_main()
        since = self.app.mark()
        chord(VK_CONTROL, VK_MENU, VK_V)
        self.app.wait_for("Retained text inserted", "Insertion failed", since=since)
        time.sleep(0.5)
        assert self.ui.read() == expected, f"Field contains {self.ui.read()!r}"

    def held_modifier(self):
        expected = "Inserted after release"
        self.provider.text = expected
        self.ui.focus_main()
        since = self.app.mark()
        toggle()
        self.app.wait_for("Recording…", since=since)
        time.sleep(1.5)
        key(VK_CONTROL)
        key(VK_MENU)
        key(VK_SPACE)
        key(VK_SPACE, up=True)
        line = self.app.wait_for("Release the shortcut keys before insertion", "Inserted", since=since)
        key(VK_MENU, up=True)
        key(VK_CONTROL, up=True)
        assert "Release the shortcut keys" in line, line
        assert self.ui.read() == "", "Text inserted while modifiers were held"
        since = self.app.mark()
        chord(VK_CONTROL, VK_MENU, VK_V)
        self.app.wait_for("Retained text inserted", since=since)
        time.sleep(0.5)
        assert self.ui.read() == expected

    def second_instance(self):
        second = subprocess.run([self.args.exe, "--config", str(self.config), "run"],
                                capture_output=True, text=True, timeout=15)
        assert second.returncode != 0, "Second instance kept running"
        assert "already running" in second.stderr, second.stderr
        assert self.app.proc.poll() is None, "First instance exited"

    def paste_restores_clipboard(self):
        self.use_config(insertion="paste")
        expected = "Pasted: Grüße 🎤 café"
        self.provider.text = expected
        sentinel = f"saykey-e2e-paste-sentinel-{time.time()}"
        self.ui.clipboard(sentinel)
        self.ui.focus_main()
        since = self.record()
        line = self.app.wait_for("Inserted", "failed", "retained", since=since)
        assert "Inserted" in line, line
        time.sleep(0.5)
        assert self.ui.read() == expected, f"Field contains {self.ui.read()!r}"
        assert self.ui.clipboard() == sentinel, "Clipboard text was not restored"

    def notepad_paste(self):
        self.use_config(insertion="paste")
        self.notepad("Notepad paste: Grüße 🎤 café, a longer sentence pasted at once.")

    def use_config(self, **extra):
        self.app.stop()
        config = self.workdir / "variant.toml"
        self.write_config(config, **extra)
        self.app = self.launch(config)

    def notepad(self, expected="Notepad typing: Grüße 🎤 café, and a longer sentence typed."):
        self.provider.text = expected
        path = self.workdir / "notepad-e2e.txt"
        path.write_text("", "utf-8")
        proc = subprocess.Popen(["notepad.exe", str(path)])
        hwnd = 0
        for _ in range(50):
            time.sleep(0.2)
            hwnd = user32.FindWindowW("Notepad", None)
            if hwnd:
                break
        assert hwnd, "Notepad did not open"
        time.sleep(1.5)
        assert focus(hwnd), "Could not focus Notepad"
        since = self.record()
        line = self.app.wait_for("Inserted", "failed", "retained", since=since)
        assert "Inserted" in line, line
        time.sleep(0.5)
        chord(VK_CONTROL, VK_S)
        time.sleep(1)
        content = path.read_text("utf-8-sig")
        user32.PostMessageW(hwnd, WM_CLOSE, 0, 0)
        proc.wait(timeout=10)
        assert content == expected, f"Notepad saved {content!r}"

    def recording_limit(self):
        self.app.stop()
        config = self.workdir / "limit.toml"
        self.write_config(config, max_recording_seconds=2)
        self.app = self.launch(config)
        self.provider.text = "Stopped by the limit"
        self.ui.focus_main()
        since = self.app.mark()
        toggle()
        self.app.wait_for("Recording…", since=since)
        if self.args.play:
            winsound.PlaySound(self.args.play, winsound.SND_FILENAME | winsound.SND_ASYNC)
        self.app.wait_for("Recording limit reached", since=since, timeout=6)
        self.app.wait_for("Inserted", since=since)
        time.sleep(0.5)
        assert self.ui.read() == "Stopped by the limit"

    def live(self):
        config = self.workdir / "live.toml"
        config.write_text('base_url = "https://api.openai.com/v1"\n'
                          'api_key_env = "OPENAI_API_KEY"\ncleanup = true\n', "utf-8")
        self.app.stop()
        self.app = self.launch(config)
        self.ui.focus_main()
        since = self.record(seconds=7)
        line = self.app.wait_for("Inserted", "failed", "discarded", "retained", since=since, timeout=60)
        time.sleep(0.5)
        print(f"  live status: {line}\n  live text:   {self.ui.read()!r}")
        assert "Inserted" in line, line

    def run_all(self):
        for name in ["insert_unicode", "hold_to_talk", "cancel_recording", "cancel_processing", "provider_error",
                     "focus_change_then_recover", "held_modifier", "second_instance", "notepad",
                     "paste_restores_clipboard", "notepad_paste", "recording_limit"] + (["live"] if self.args.live else []):
            print(f"- {name} …", flush=True)
            self.check(name, getattr(self, name))
            print(f"  {self.results[-1][1]} {self.results[-1][2]}", flush=True)
        self.app.stop()


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--exe", default=str(Path(__file__).resolve().parents[1] / "target/release/saykey.exe"))
    parser.add_argument("--play", help="WAV file to play through the speakers while recording")
    parser.add_argument("--audio", help="16-bit WAV replacing the microphone (test-audio builds)")
    parser.add_argument("--live", action="store_true", help="also dictate through the real provider")
    args = parser.parse_args()
    args.exe = str(Path(args.exe).resolve())
    if sys.platform != "win32":
        sys.exit("This end-to-end check requires Windows.")
    ui = Ui()
    provider = Provider()
    outcome = {}

    def worker():
        original = ui.clipboard()
        with tempfile.TemporaryDirectory(prefix="saykey-e2e-") as workdir:
            run = None
            try:
                run = Run(args, ui, provider, Path(workdir))
                run.run_all()
            except Exception as error:  # noqa: BLE001
                outcome["error"] = error
            finally:
                if run:
                    run.app.stop()
                    outcome["results"] = run.results
                if original is not None:
                    ui.clipboard(original)
                ui.root.after(0, ui.root.destroy)

    threading.Thread(target=worker, daemon=True).start()
    ui.root.mainloop()
    results = outcome.get("results", [])
    print("\n| Scenario | Result | Detail |\n| --- | --- | --- |")
    for name, status, detail in results:
        print(f"| {name} | {status} | {detail[:200]} |")
    if "error" in outcome:
        print(f"Harness error: {outcome['error']}")
    failed = "error" in outcome or any(status != "PASS" for _, status, _ in results)
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
