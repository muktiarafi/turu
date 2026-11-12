#!/usr/bin/env python3
"""Removes the MethodParameters attributes from the class files under a folder (air/ane/build.sh).

The AIR SDK's own d8 (1.2.52 in 51.5.1.1), which ADT can dex the extension's jar with, crashes on
an entry without a name ("Compilation failed with an internal error", a NullPointerException in
JarClassFileReader.visitParameter), and Sentry's classes have some (io.sentry.JsonObjectWriter,
anonymous classes). The attribute only names parameters for reflection, which nothing in the
extension does. Usage: strip-parameters.py <classes folder>
"""

import os
import struct
import sys


def strip(data):
    """The class without MethodParameters attributes, or None if it had none (or isn't one)."""
    if data[:4] != b"\xca\xfe\xba\xbe":
        return None
    pos = 10
    count = struct.unpack(">H", data[8:10])[0]
    utf8 = {}
    index = 1
    while index < count:
        tag = data[pos]
        if tag == 1:
            length = struct.unpack(">H", data[pos + 1 : pos + 3])[0]
            utf8[index] = data[pos + 3 : pos + 3 + length]
            pos += 3 + length
        elif tag in (3, 4):
            pos += 5
        elif tag in (5, 6):
            pos += 9
            index += 1
        elif tag in (7, 8, 16, 19, 20):
            pos += 3
        elif tag in (9, 10, 11, 12, 17, 18):
            pos += 5
        elif tag == 15:
            pos += 4
        else:
            raise ValueError(f"constant pool tag {tag}")
        index += 1
    out = bytearray(data[:pos])
    # access, this, super, interfaces
    interfaces = struct.unpack(">H", data[pos + 6 : pos + 8])[0]
    end = pos + 8 + 2 * interfaces
    out += data[pos:end]
    pos = end
    removed = 0
    for _ in ("fields", "methods"):
        members = struct.unpack(">H", data[pos : pos + 2])[0]
        out += data[pos : pos + 2]
        pos += 2
        for _ in range(members):
            out += data[pos : pos + 6]  # access, name, descriptor
            attributes = struct.unpack(">H", data[pos + 6 : pos + 8])[0]
            pos += 8
            kept = []
            for _ in range(attributes):
                name = struct.unpack(">H", data[pos : pos + 2])[0]
                length = struct.unpack(">I", data[pos + 2 : pos + 6])[0]
                attribute = data[pos : pos + 6 + length]
                pos += 6 + length
                if utf8.get(name) == b"MethodParameters":
                    removed += 1
                else:
                    kept.append(attribute)
            out += struct.pack(">H", len(kept))
            for attribute in kept:
                out += attribute
    out += data[pos:]  # the class's own attributes
    return bytes(out) if removed else None


def main():
    folder = sys.argv[1]
    changed = 0
    for directory, _, files in os.walk(folder):
        for file in files:
            if not file.endswith(".class"):
                continue
            path = os.path.join(directory, file)
            with open(path, "rb") as f:
                stripped = strip(f.read())
            if stripped is not None:
                with open(path, "wb") as f:
                    f.write(stripped)
                changed += 1
    print(f"MethodParameters removed from {changed} classes")


if __name__ == "__main__":
    main()
