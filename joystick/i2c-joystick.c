#include "i2c-joystick.h"
#include <i2c/smbus.h>

int open_i2c_device(char *filename) {
    int file;

    if ((file = open(filename, O_RDWR)) < 0) {
        // TODO: check errno
        fprintf(stderr, "Failed to open I2C bus at %s\n", filename);
        exit(1);
    }

    return file;
}

int read_i2c_joystick(int file, int i2c_addr, I2CJoystickValues *values) {
    if (ioctl(file, I2C_SLAVE, i2c_addr) < 0) {
        // TODO: check errno
        fprintf(stderr, "Failed to talk to I2C joystick at 0x%x\n", i2c_addr);
        return -1;
    }

    int32_t res;

    // Read X axis
    res = i2c_smbus_read_word_data(file, X_AXIS_REGISTER);
    if (res < 0) {
        fprintf(stderr, "Failed to get I2C joystick X-axis at 0x%x\n", i2c_addr);
        return -1;
    }
    values->x_axis = (int16_t)((uint16_t)(res & 0xffff));

    // Read Y axis
    res = i2c_smbus_read_word_data(file, Y_AXIS_REGISTER);
    if (res < 0) {
        fprintf(stderr, "Failed to get I2C joystick Y-axis at 0x%x\n", i2c_addr);
        return -1;
    }
    values->y_axis = (int16_t)((uint16_t)(res & 0xffff));

    // Read button value
    res = i2c_smbus_read_byte_data(file, BUTTON_REGISTER);
    if (res < 0) {
        fprintf(stderr, "Failed to get I2C joystick button at 0x%x\n", i2c_addr);
        return -1;
    }
    // Inverted, in register 1 is unpressed, 0 is pressed
    values->button = res ? 0 : 1;

    return 0;
}

int set_i2c_joystick_color(int file, int i2c_addr, uint32_t color) {
    if (ioctl(file, I2C_SLAVE, i2c_addr) < 0) {
        // TODO: check errno
        fprintf(stderr, "Failed to talk to I2C joystick at 0x%x\n", i2c_addr);
        return -1;
    }

    int32_t res;
    uint8_t *rgb = (uint8_t *)&color;

    for (uint8_t i = 0; i < 3; i++) {
        res = i2c_smbus_write_byte_data(file, LED_RGB_REGISTER + i, rgb[i]);
        if (res < 0) {
            // TODO: check errno
            fprintf(stderr, "Failed to set I2C joystick LED at 0x%x\n", i2c_addr);
            return -1;
        }
    }

    return 0;
}
