#!/usr/bin/env python3
"""A scripted shell for Turu's client mode (docs/TAB_PROCESSES_PROPOSAL.md, M2).

Starts `turu --client=<address>` with a token, shows its tab at a size, runs a few steps (waits,
clicks, keys, picture dumps to PNG) and reports the pictures it got. No window: for checking the
client end to end where nobody can look at a screen.

Usage: tools/shell/fake_shell.py <turu binary> <out dir> [steps...]
Steps: show:<w>x<h>[@<scale>]  wait:<seconds>  click:<x>,<y>  key:<egui name>  text:<text>
       dump:<name>  hide   (default: show:1280x720 wait:25 dump:login click:40,90 wait:2
       dump:menu)
"""
import json
import os
import secrets
import socket
import struct
import subprocess
import sys
import threading
import time


def send(sock, message):
    data = json.dumps(message).encode()
    sock.sendall(struct.pack(">I", len(data)) + data)


def recv_exact(sock, n):
    chunks = []
    while n:
        chunk = sock.recv(min(n, 1 << 20))
        if not chunk:
            raise EOFError
        chunks.append(chunk)
        n -= len(chunk)
    return b"".join(chunks)


def recv(sock):
    (length,) = struct.unpack(">I", recv_exact(sock, 4))
    return json.loads(recv_exact(sock, length))


def main():
    turu, out = sys.argv[1], sys.argv[2]
    steps = sys.argv[3:] or ["show:1280x720", "wait:25", "dump:login", "click:40,90", "wait:2", "dump:menu"]
    os.makedirs(out, exist_ok=True)
    listener = socket.socket()
    listener.bind(("127.0.0.1", 0))
    listener.listen(1)
    address = "%s:%d" % listener.getsockname()
    token = secrets.token_hex(24)
    env = dict(os.environ, TURU_SHELL_TOKEN=token)
    log = open(os.path.join(out, "client.log"), "w")
    client = subprocess.Popen([turu, "--client=" + address], env=env, stdout=log, stderr=subprocess.STDOUT)
    listener.settimeout(60)
    sock, _ = listener.accept()
    hello = recv(sock)
    assert hello.get("type") == "hello" and hello.get("token") == token, hello
    print("client %s (Turu %s) connected" % (hello["pid"], hello["version"]))

    frames = []
    urls = []

    def reader():
        try:
            while True:
                message = recv(sock)
                if message["type"] == "frame":
                    recv_exact(sock, message["len"])
                    frames.append((time.time(), message["width"], message["height"]))
                elif message["type"] == "open_url":
                    urls.append(message["url"])
        except (EOFError, OSError):
            pass

    threading.Thread(target=reader, daemon=True).start()
    started = time.time()
    for step in steps:
        kind, _, arg = step.partition(":")
        if kind == "show":
            size, _, scale = arg.partition("@")
            width, height = (int(v) for v in size.split("x"))
            send(sock, {"type": "show", "width": width, "height": height, "scale": float(scale or 1)})
        elif kind == "hide":
            send(sock, {"type": "hide"})
        elif kind == "wait":
            time.sleep(float(arg))
        elif kind == "click":
            x, y = (float(v) for v in arg.split(","))
            for pointer in ("move", "down", "up"):
                send(sock, {"type": "pointer", "kind": pointer, "button": "left", "x": x, "y": y})
                time.sleep(0.05)
        elif kind == "key":
            for pressed in (True, False):
                send(sock, {"type": "key", "key": arg, "pressed": pressed, "mods": {}})
        elif kind == "text":
            send(sock, {"type": "text", "text": arg})
        elif kind == "dump":
            path = os.path.abspath(os.path.join(out, arg + ".png"))
            send(sock, {"type": "dump", "path": path})
            # A picture comes when the client next draws: nudge the UI so it does.
            send(sock, {"type": "pointer", "kind": "move", "button": "left", "x": 1, "y": 1})
            time.sleep(1.5)
            print("dump %s: %s" % (arg, "written" if os.path.exists(path) else "MISSING"))
        print("%6.1f s  %s  (%d pictures so far)" % (time.time() - started, step, len(frames)))
    if len(frames) > 1:
        span = frames[-1][0] - frames[0][0]
        print("pictures: %d, last %dx%d, %.1f a second" % (len(frames), frames[-1][1], frames[-1][2], (len(frames) - 1) / max(span, 0.001)))
    else:
        print("pictures: %d" % len(frames))
    sock.close()
    try:
        client.wait(timeout=15)
        print("client quit with the shell (status %s)" % client.returncode)
    except subprocess.TimeoutExpired:
        print("client still running 15 s after the shell closed: killing it")
        client.kill()


if __name__ == "__main__":
    main()
