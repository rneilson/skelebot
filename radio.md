# Command protocol

- Variable-length packet
- One-byte command/telemetry type
- 0-3 bytes command/telemetry payload

### Commands

| Type | Length | Description         | Payload values   |
|------|--------|---------------------|------------------|
| 0xF0 |    0   | No-op               | None             |
| 0xF1 |    1   | Change channel      | Channel 0-126    |
| 0xF2 |  N/A   | (Reserved)          | N/A              |
| 0xF3 |    0   | Stop                | None             |
| 0xF4 |    2   | Forward (L+, R+)    | L 0-100, R 0-100 |
| 0xF5 |    2   | Turn right (L+, R-) | L 0-100, R 0-100 |
| 0xF6 |    2   | Turn left (L-, R+)  | L 0-100, R 0-100 |
| 0xF7 |    2   | Backward (L-, R-)   | L 0-100, R 0-100 |
| 0xF8 |  N/A   | (Reserved)          | N/A              |
| 0xF9 |  N/A   | (Reserved)          | N/A              |

### Telemetry

| Type | Length | Description         | Payload values   |
|------|--------|---------------------|------------------|
| 0xFA |    0   | No-op               | None             |
| 0xFB |    2   | Battery voltage     | Voltage (u16 BE) |
| 0xFC |    2   | Left RPM            | RPM (u16 BE)     |
| 0xFD |    2   | Right RPM           | RPM (u16 BE)     |
| 0xFE |  N/A   | (Reserved)          | N/A              |
| 0xFF |  N/A   | (Reserved)          | N/A              |

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
