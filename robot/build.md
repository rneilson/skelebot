# Building

## Placeholder

From `robot/` subdir:

```
arduino-cli compile -b arduino:avr:pro --board-options cpu=8MHzatmega328 ./placeholder/
arduino-cli upload -p /dev/ttyUSB0 -b arduino:avr:pro --board-options cpu=8MHzatmega328 ./placeholder/
```
