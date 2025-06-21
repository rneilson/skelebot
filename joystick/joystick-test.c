// Some code borrowed from https://github.com/piloChambert/RPI-I2C-Joystick/tree/master/driver

#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <signal.h>
#include <sys/ioctl.h>
#include <unistd.h>

#include "i2c-joystick.h"

#define I2C_LEFT_STICK_ADDR 0x63
#define I2C_RIGHT_STICK_ADDR 0x64

#define LED_LEFT_COLOR 0x00000f00 // green
#define LED_RIGHT_COLOR 0x000f0f00 // yellow

#define UPDATE_MICROSECONDS 50000 // µs (20Hz)

#define I2C_BUS_PATH "/dev/i2c-8" // TODO: make cmdline arg

static volatile int running = 1;

void sigint_handler(int unused) {
    running = 0;
}

int main(int argc, char *argv[]) {
    int i2c_file = open_i2c_device(I2C_BUS_PATH);
    fprintf(stdout, "Connected to I2C bus at %s\n", I2C_BUS_PATH);
    fprintf(stdout, "\033[?25l");
    fflush(stdout);

    // Set joystick LED colors to tell them apart
    if (set_i2c_joystick_color(i2c_file, I2C_LEFT_STICK_ADDR, LED_LEFT_COLOR) < 0) {
        exit(1);
    }
    if (set_i2c_joystick_color(i2c_file, I2C_RIGHT_STICK_ADDR, LED_RIGHT_COLOR) < 0) {
        exit(1);
    }

    I2CJoystickValues left_stick;
    left_stick.x_axis = 0;
    left_stick.y_axis = 0;
    left_stick.button = 0;
    I2CJoystickValues right_stick;
    right_stick.x_axis = 0;
    right_stick.y_axis = 0;
    right_stick.button = 0;
    
    int res;

    signal(SIGINT, sigint_handler);

    while(running) {
        usleep(UPDATE_MICROSECONDS);

        res = read_i2c_joystick(i2c_file, I2C_LEFT_STICK_ADDR, &left_stick);
        if (res < 0) {
            fprintf(stderr, "Couldn't read left joystick, skipping update\n");
            continue;
        }

        res = read_i2c_joystick(i2c_file, I2C_RIGHT_STICK_ADDR, &right_stick);
        if (res < 0) {
            fprintf(stderr, "Couldn't read right joystick, skipping update\n");
            continue;
        }

        fprintf(stdout, "\rL: X %+5d Y %+5d B %1u  R: X %+5d Y %+5d B %1u",
            left_stick.x_axis, left_stick.y_axis, left_stick.button,
            right_stick.x_axis, right_stick.y_axis, right_stick.button);
        fflush(stdout);
    }

    // Clear joystick colors
    if (set_i2c_joystick_color(i2c_file, I2C_LEFT_STICK_ADDR, 0) < 0) {
        exit(1);
    }
    if (set_i2c_joystick_color(i2c_file, I2C_RIGHT_STICK_ADDR, 0) < 0) {
        exit(1);
    }

    close(i2c_file);
    fprintf(stdout, "\nExiting...\n");
    fprintf(stdout, "\033[?25h");
    fflush(stdout);
}
