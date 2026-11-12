#!/usr/bin/env python3
"""A virtual Xbox 360 controller for testing the launcher without hardware.

Linux only; needs write access to /dev/uinput. SDL recognises it as an
Xbox 360 controller (VID 045E, PID 028E), so the launcher reads it with
its gamepad layout.

    fake-pad.py [--script FILE] [--seconds N]

The script holds lines "<seconds> <stick_x> <right_trigger> [buttons]":
stick_x is -1..1, the trigger 0..1, buttons a comma list of names (a, b,
x, y, lb, rb, back, start, guide, ls, rs, and up, down, left, right on
the d-pad) held from that time on.
"""
import argparse
import fcntl
import json
import os
import struct
import sys
import time

EV_SYN, EV_KEY, EV_ABS = 0x00, 0x01, 0x03
ABS_X, ABS_Y, ABS_Z, ABS_RX, ABS_RY, ABS_RZ, ABS_HAT0X, ABS_HAT0Y = 0, 1, 2, 3, 4, 5, 0x10, 0x11
BUTTONS = {"a": 0x130, "b": 0x131, "x": 0x133, "y": 0x134, "lb": 0x136, "rb": 0x137,
           "back": 0x13A, "start": 0x13B, "guide": 0x13C, "ls": 0x13D, "rs": 0x13E}


def _IOC(d, t, nr, size):
    return (d << 30) | (size << 16) | (ord(t) << 8) | nr


UI_DEV_CREATE = _IOC(0, "U", 1, 0)
UI_DEV_DESTROY = _IOC(0, "U", 2, 0)
UI_DEV_SETUP = _IOC(1, "U", 3, 92)
UI_ABS_SETUP = _IOC(1, "U", 4, 28)
UI_SET_EVBIT = _IOC(1, "U", 100, 4)
UI_SET_KEYBIT = _IOC(1, "U", 101, 4)
UI_SET_ABSBIT = _IOC(1, "U", 103, 4)
INPUT_EVENT = "llHHi"  # struct input_event on 64-bit


def emit(fd, etype, code, value):
    os.write(fd, struct.pack(INPUT_EVENT, 0, 0, etype, code, value))


def log(**kw):
    kw["t"] = round(time.monotonic(), 3)
    print(json.dumps(kw), flush=True)


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
            buttons = parts[3].split(",") if len(parts) > 3 else []
            steps.append((float(parts[0]), float(parts[1]), float(parts[2]), buttons))

    fd = os.open("/dev/uinput", os.O_RDWR | os.O_NONBLOCK)
    for ev in (EV_KEY, EV_ABS):
        fcntl.ioctl(fd, UI_SET_EVBIT, ev)
    for code in BUTTONS.values():
        fcntl.ioctl(fd, UI_SET_KEYBIT, code)
    for code, lo, hi in ((ABS_X, -32768, 32767), (ABS_Y, -32768, 32767), (ABS_RX, -32768, 32767),
                         (ABS_RY, -32768, 32767), (ABS_Z, 0, 255), (ABS_RZ, 0, 255),
                         (ABS_HAT0X, -1, 1), (ABS_HAT0Y, -1, 1)):
        fcntl.ioctl(fd, UI_SET_ABSBIT, code)
        fcntl.ioctl(fd, UI_ABS_SETUP, struct.pack("<Hxxiiiiii", code, 0, lo, hi, 0, 0, 0))
    name = b"Microsoft X-Box 360 pad (virtual)"
    fcntl.ioctl(fd, UI_DEV_SETUP, struct.pack("<HHHH80sI", 0x03, 0x045E, 0x028E, 0x0114, name, 0))
    fcntl.ioctl(fd, UI_DEV_CREATE)
    time.sleep(0.3)
    log(event="created")

    start = time.monotonic()
    current = None
    try:
        while time.monotonic() - start < args.seconds:
            now = time.monotonic() - start
            due = [s for s in steps if s[0] <= now]
            if due and due[-1] != current:
                current = due[-1]
                _, stick, trigger, buttons = current
                emit(fd, EV_ABS, ABS_X, int(max(-1, min(1, stick)) * 32767))
                emit(fd, EV_ABS, ABS_RZ, int(max(0, min(1, trigger)) * 255))
                for b, code in BUTTONS.items():
                    emit(fd, EV_KEY, code, 1 if b in buttons else 0)
                hat_x = ("right" in buttons) - ("left" in buttons)
                hat_y = ("down" in buttons) - ("up" in buttons)
                emit(fd, EV_ABS, ABS_HAT0X, hat_x)
                emit(fd, EV_ABS, ABS_HAT0Y, hat_y)
                emit(fd, EV_SYN, 0, 0)
                log(event="input", stick=stick, trigger=trigger, buttons=buttons)
            time.sleep(0.002)
    finally:
        fcntl.ioctl(fd, UI_DEV_DESTROY)
        os.close(fd)
        log(event="destroyed")


if __name__ == "__main__":
    sys.exit(main())
