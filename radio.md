# Command protocol

- Variable-length packet
- One-byte command/telemetry type
- 0-3 bytes command/telemetry payload

### Commands

| Type | Length | Description         | Payload values   | Notes                   |
|------|--------|---------------------|------------------|-------------------------|
| 0xF0 |    0   | No-op               | None             |                         |
| 0xF1 |    1   | Change channel      | Channel 0-126    |                         |
| 0xF2 |  N/A   | (Reserved)          | N/A              |                         |
| 0xF3 |    0   | Stop                | None             |                         |
| 0xF4 |    2   | Drive (L, R)        | L 0-200, R 0-200 | (0, 200) -> (-100, 100) |
| 0xF5 |    0   | Center camera       | None             |                         |
| 0xF6 |    2   | Look (Pan, Tilt)    | P 0-180, T 0-180 | (0, 180) -> (-90, 90)   |
| 0xF7 |  N/A   | (Reserved)          | N/A              |                         |

### Telemetry

| Type | Length | Description         | Payload values   | Notes                   |
|------|--------|---------------------|------------------|-------------------------|
| 0xF8 |    0   | No-op               | None             |                         |
| 0xF9 |  N/A   | (Reserved)          | N/A              |                         |
| 0xFA |  N/A   | (Reserved)          | N/A              |                         |
| 0xFB |    2   | Battery voltage     | Voltage (u16 BE) | Vbat / 1023.0           |
| 0xFC |    2   | Battery current     | Current (u16 BE) | Ibat / 1023.0           |
| 0xFD |    2   | Left RPM            | RPM (u16 BE)     |                         |
| 0xFE |    2   | Right RPM           | RPM (u16 BE)     |                         |
| 0xFF |  N/A   | (Reserved)          | N/A              |                         |

## Radio channels

See:
- https://www.allaboutcircuits.com/uploads/articles/Bluetooth_and_WLAN_frequencies.jpg
- https://devzone.nordicsemi.com/f/nordic-q-a/38005/nrf24l01-communication-is-interfered-by-wifi

| MHz       | Channels                               |
|-----------|----------------------------------------|
| 2400-2402 | 0, 1                                   |
| 2421-2427 | 21, 22, 23, 24, 25, 26, 27             |
| 2445-2453 | 45, 46, 47, 48, 49, 50, 51, 52, 53     |
| 2471-2480 | 71, 72, 73, 74, 75, 76, 77, 78, 79, 80 |
