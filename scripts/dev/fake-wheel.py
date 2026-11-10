#!/usr/bin/env python3
"""A virtual force-feedback wheel for testing the launcher without hardware.

Linux only; needs write access to /dev/uinput. It shows up as a
"FANATEC CSL DD (virtual)" (VID 0EB7, PID 0020) with steering (ABS_X),
accelerator (ABS_Y) and brake (ABS_Z) pedals that rest at the top of
their travel, 12 buttons and a hat, and constant, damper and friction
force feedback. Every effect upload, play and gain/autocentre change is
printed as one JSON line, so a test can check what the launcher sends.

    fake-wheel.py [--script FILE] [--seconds N]

The script holds lines "<seconds> <steer> <accel> <brake> [buttons]":
steer is -1..1, pedals 0..1, buttons a comma list of numbers held from
that time on.
"""
import argparse
import fcntl
import json
import os
import select
import struct
import sys
import time

EV_SYN, EV_KEY, EV_ABS, EV_FF, EV_UINPUT = 0x00, 0x01, 0x03, 0x15, 0x0101
ABS_X, ABS_Y, ABS_Z, ABS_HAT0X, ABS_HAT0Y = 0x00, 0x01, 0x02, 0x10, 0x11
BTN_TRIGGER = 0x120
FF_CONSTANT, FF_FRICTION, FF_DAMPER, FF_GAIN, FF_AUTOCENTER = 0x52, 0x54, 0x55, 0x60, 0x61
UI_FF_UPLOAD, UI_FF_ERASE = 1, 2


def _IOC(d, t, nr, size):
    return (d << 30) | (size << 16) | (ord(t) << 8) | nr


UI_DEV_CREATE = _IOC(0, "U", 1, 0)
UI_DEV_DESTROY = _IOC(0, "U", 2, 0)
UI_DEV_SETUP = _IOC(1, "U", 3, 92)
UI_ABS_SETUP = _IOC(1, "U", 4, 28)
UI_SET_EVBIT = _IOC(1, "U", 100, 4)
UI_SET_KEYBIT = _IOC(1, "U", 101, 4)
UI_SET_ABSBIT = _IOC(1, "U", 103, 4)
UI_SET_FFBIT = _IOC(1, "U", 107, 4)
FF_EFFECT_SIZE = 48
UPLOAD_SIZE = 8 + 2 * FF_EFFECT_SIZE
UI_BEGIN_FF_UPLOAD = _IOC(3, "U", 200, UPLOAD_SIZE)
UI_END_FF_UPLOAD = _IOC(1, "U", 201, UPLOAD_SIZE)
UI_BEGIN_FF_ERASE = _IOC(3, "U", 202, 12)
UI_END_FF_ERASE = _IOC(1, "U", 203, 12)
INPUT_EVENT = "llHHi"  # struct input_event on 64-bit


def emit(fd, etype, code, value):
    os.write(fd, struct.pack(INPUT_EVENT, 0, 0, etype, code, value))


def log(**kw):
    kw["t"] = round(time.monotonic(), 3)
    print(json.dumps(kw), flush=True)


def decode_effect(raw):
    etype, eid, direction = struct.unpack_from("<HhH", raw, 0)
    out = {"type": {FF_CONSTANT: "constant", FF_DAMPER: "damper", FF_FRICTION: "friction"}.get(etype, hex(etype)),
           "id": eid, "direction": direction}
    u = raw[16:]
    if etype == FF_CONSTANT:
        out["level"] = struct.unpack_from("<h", u, 0)[0]
    elif etype in (FF_DAMPER, FF_FRICTION):
        rs, ls, rc, lc, db, center = struct.unpack_from("<HHhhHh", u, 0)
        out["coeff"] = rc
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--script")
    ap.add_argument("--seconds", type=float, default=30)
    args = ap.parse_args()
    steps = []
    if args.script:
        for line in open(args.script):
            parts = line.split()
            if not parts or parts[0].startswith("#"):
                continue
            buttons = [int(b) for b in parts[4].split(",")] if len(parts) > 4 else []
            steps.append((float(parts[0]), float(parts[1]), float(parts[2]), float(parts[3]), buttons))

    fd = os.open("/dev/uinput", os.O_RDWR | os.O_NONBLOCK)
    for ev in (EV_KEY, EV_ABS, EV_FF):
        fcntl.ioctl(fd, UI_SET_EVBIT, ev)
    for b in range(12):
        fcntl.ioctl(fd, UI_SET_KEYBIT, BTN_TRIGGER + b)
    for ff in (FF_CONSTANT, FF_DAMPER, FF_FRICTION, FF_GAIN, FF_AUTOCENTER):
        fcntl.ioctl(fd, UI_SET_FFBIT, ff)
    for code, lo, hi in ((ABS_X, -32768, 32767), (ABS_Y, -32768, 32767), (ABS_Z, -32768, 32767),
                         (ABS_HAT0X, -1, 1), (ABS_HAT0Y, -1, 1)):
        fcntl.ioctl(fd, UI_SET_ABSBIT, code)
        fcntl.ioctl(fd, UI_ABS_SETUP, struct.pack("<Hxxiiiiii", code, 0, lo, hi, 0, 0, 0))
    name = b"FANATEC CSL DD (virtual)"
    fcntl.ioctl(fd, UI_DEV_SETUP, struct.pack("<HHHH80sI", 0x03, 0x0EB7, 0x0020, 0x0111, name, 16))
    fcntl.ioctl(fd, UI_DEV_CREATE)
    time.sleep(0.3)
    # pedals rest at the top of their travel
    emit(fd, EV_ABS, ABS_Y, 32767)
    emit(fd, EV_ABS, ABS_Z, 32767)
    emit(fd, EV_SYN, 0, 0)
    log(event="created")

    start = time.monotonic()
    current = None
    try:
        while time.monotonic() - start < args.seconds:
            now = time.monotonic() - start
            due = [s for s in steps if s[0] <= now]
            if due and due[-1] != current:
                current = due[-1]
                _, steer, accel, brake, buttons = current
                emit(fd, EV_ABS, ABS_X, int(max(-1, min(1, steer)) * 32767))
                emit(fd, EV_ABS, ABS_Y, int(32767 - max(0, min(1, accel)) * 65535))
                emit(fd, EV_ABS, ABS_Z, int(32767 - max(0, min(1, brake)) * 65535))
                for b in range(12):
                    emit(fd, EV_KEY, BTN_TRIGGER + b, 1 if b in buttons else 0)
                emit(fd, EV_SYN, 0, 0)
                log(event="input", steer=steer, accel=accel, brake=brake, buttons=buttons)
            r, _, _ = select.select([fd], [], [], 0.01)
            if not r:
                continue
            try:
                data = os.read(fd, struct.calcsize(INPUT_EVENT))
            except BlockingIOError:
                continue
            _, _, etype, code, value = struct.unpack(INPUT_EVENT, data)
            if etype == EV_UINPUT and code == UI_FF_UPLOAD:
                buf = bytearray(struct.pack("<Ii", value, 0) + bytes(2 * FF_EFFECT_SIZE))
                fcntl.ioctl(fd, UI_BEGIN_FF_UPLOAD, buf)
                log(event="upload", **decode_effect(bytes(buf[8:8 + FF_EFFECT_SIZE])))
                struct.pack_into("<i", buf, 4, 0)
                fcntl.ioctl(fd, UI_END_FF_UPLOAD, bytes(buf))
            elif etype == EV_UINPUT and code == UI_FF_ERASE:
                buf = bytearray(struct.pack("<Iii", value, 0, 0))
                fcntl.ioctl(fd, UI_BEGIN_FF_ERASE, buf)
                log(event="erase", id=struct.unpack_from("<i", buf, 8)[0])
                fcntl.ioctl(fd, UI_END_FF_ERASE, bytes(buf))
            elif etype == EV_FF:
                if code == FF_GAIN:
                    log(event="gain", value=value)
                elif code == FF_AUTOCENTER:
                    log(event="autocenter", value=value)
                else:
                    log(event="play", id=code, value=value)
    finally:
        fcntl.ioctl(fd, UI_DEV_DESTROY)
        os.close(fd)
        log(event="destroyed")


if __name__ == "__main__":
    sys.exit(main())
