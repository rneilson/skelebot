// Some code borrowed from https://github.com/piloChambert/RPI-I2C-Joystick/tree/master/driver

#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <signal.h>
#include <sys/ioctl.h>
#include <unistd.h>

#include "evdev-joystick.h"
#include "i2c-joystick.h"

#define I2C_LEFT_STICK_ADDR 0x63
#define I2C_RIGHT_STICK_ADDR 0x64

#define LED_LEFT_COLOR 0x00000f00 // green
#define LED_RIGHT_COLOR 0x000f0f00 // yellow

#define UPDATE_MICROSECONDS 50000 // µs (20Hz)

#define JOYSTICK_AXIS_MIN -4096
#define JOYSTICK_AXIS_MAX 4096

#define I2C_BUS_PATH "/dev/i2c-8" // TODO: make cmdline arg

static volatile int running = 1;

void sigint_handler(int unused) {
    running = 0;
}

static volatile int i2c_file = 0;

void i2c_file_closer() {
    if (i2c_file) {
        close(i2c_file);
        i2c_file = 0;
        fprintf(stdout, "Disconnected from I2C bus\n");
    }
}

static volatile int dev_file = 0;

void dev_file_closer() {
    if (dev_file) {
        close_joystick_device(dev_file);
        dev_file = 0;
        fprintf(stdout, "Closed joystick device\n");
    }
}

int main(int argc, char *argv[]) {
    // I2C-side setup
    i2c_file = open_i2c_device(I2C_BUS_PATH);
    atexit(i2c_file_closer);
    fprintf(stdout, "Connected to I2C bus at %s\n", I2C_BUS_PATH);

    // Set joystick LED colors to tell them apart
    if (set_i2c_joystick_color(i2c_file, I2C_LEFT_STICK_ADDR, LED_LEFT_COLOR) < 0) {
        exit(1);
    }
    if (set_i2c_joystick_color(i2c_file, I2C_RIGHT_STICK_ADDR, LED_RIGHT_COLOR) < 0) {
        exit(1);
    }

    I2CJoystickValues left_stick = {0, 0, 0};
    I2CJoystickValues right_stick = {0, 0, 0};

    // Uinput-side setup
    dev_file = setup_joystick_device(JOYSTICK_AXIS_MIN, JOYSTICK_AXIS_MAX);
    if (dev_file < 0) {
        fprintf(stderr, "Couldn't set up device, exiting...\n");
        exit(1);
    }
    atexit(dev_file_closer);
    fprintf(stdout, "Created joystick device\n");
    
    signal(SIGINT, sigint_handler);

    JoystickState joystick_state = {0, 0, 0, 0, 0, 0};

    while(running) {
        usleep(UPDATE_MICROSECONDS);

        if (read_i2c_joystick(i2c_file, I2C_LEFT_STICK_ADDR, &left_stick) < 0) {
            fprintf(stderr, "Couldn't read left joystick, skipping update\n");
            continue;
        }

        if (read_i2c_joystick(i2c_file, I2C_RIGHT_STICK_ADDR, &right_stick) < 0) {
            fprintf(stderr, "Couldn't read right joystick, skipping update\n");
            continue;
        }

        JoystickState new_state = {
            .l_x_axis = left_stick.x_axis,
            .l_y_axis = left_stick.y_axis,
            .r_x_axis = right_stick.x_axis,
            .r_y_axis = right_stick.y_axis,
            .l_button = left_stick.button,
            .r_button = right_stick.button,
        };
        // We don't currently actually need the updated bitmap
        if (update_joystick_state(dev_file, &joystick_state, &new_state) < 0) {
            fprintf(stderr, "Couldn't update joystick state\n");
        }
    }

    // Clear joystick colors
    if (set_i2c_joystick_color(i2c_file, I2C_LEFT_STICK_ADDR, 0) < 0) {
        exit(1);
    }
    if (set_i2c_joystick_color(i2c_file, I2C_RIGHT_STICK_ADDR, 0) < 0) {
        exit(1);
    }

    fprintf(stdout, "\nExiting...\n");
    exit(0);
}
