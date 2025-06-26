# Skelebot

This is a bit of a frankenproject, combining an old [SeeedStudio Hercules robot](https://wiki.seeedstudio.com/Skeleton_Bot-4WD_hercules_mobile_robotic_platform/), a slightly-old [Rockpro64 SBC](https://pine64.org/documentation/ROCKPro64), a couple [M5Stack Joystick2](https://docs.m5stack.com/en/unit/Unit-JoyStick2) units, and using a [Crazyradio PA](https://www.bitcraze.io/products/crazyradio-pa/) and an nRF24L01+ for radio control.

- `controller/`
    - Controller program written in Rust, currently a TUI interface using Ratatui
    - Assumes a two-stick joystick/gamepad available via `evdev`
    - Transmits using Crazyradio PA via USB
- `joystick/`
    - Joystick I2C userspace driver daemon in C
    - Provides joystick axes and thumbstick buttons via `uinput` synthetic device
- `robot/`
    - Robot firmware as Arduino sketch
    - Additional placeholder sketch for testing radio protocol and controls
- `radio.md`
    - Description of custom radio protocol
