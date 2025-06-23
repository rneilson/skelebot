#ifndef I2C_JOYSTICK_H
#define I2C_JOYSTICK_H

#include <stdint.h>

#define X_AXIS_REGISTER 0x50 // 2 bytes, low high
#define Y_AXIS_REGISTER 0x52 // 2 bytes, low high
#define BUTTON_REGISTER 0x20 // 1 byte
#define LED_RGB_REGISTER 0x30 // 3 bytes, blue green red (4 bytes with null)

typedef struct {
    int16_t x_axis;
    int16_t y_axis;
    uint8_t button;
} I2CJoystickValues;

int open_i2c_device(char* filename);
int read_i2c_joystick(int file, int i2c_addr, I2CJoystickValues *values);
int set_i2c_joystick_color(int file, int i2c_addr, uint32_t color);

#endif // I2C_JOYSTICK_H
