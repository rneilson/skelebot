// Some code borrowed from https://github.com/piloChambert/RPI-I2C-Joystick/tree/master/driver

#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <signal.h>
#include <sys/ioctl.h>
#include <sys/timerfd.h>
#include <time.h>
#include <unistd.h>

#include "evdev-joystick.h"
#include "i2c-joystick.h"

#define I2C_LEFT_STICK_ADDR 0x63
#define I2C_RIGHT_STICK_ADDR 0x64

#define LED_LEFT_COLOR 0x00000f00 // green
#define LED_RIGHT_COLOR 0x000f0f00 // yellow

#define UPDATE_MICROSECONDS 50000 // µs (20Hz)

#define JOYSTICK_AXIS_FUZZ 16
#define JOYSTICK_AXIS_FLAT 16

#define JOYSTICK_AXIS_I2C_MIN -4095
#define JOYSTICK_AXIS_I2C_MAX 4095

#define I2C_BUS_PATH "/dev/i2c-8" // TODO: make cmdline arg

static volatile int running = 1;

void sig_exit_handler(int unused) {
    running = 0;
}

static volatile int i2c_file = 0;

void i2c_file_closer() {
    if (i2c_file) {
        close(i2c_file);
        i2c_file = 0;
        fprintf(stdout, "Disconnected from I2C bus\n");
        fflush(stdout);
    }
}

static volatile int dev_file = 0;

void dev_file_closer() {
    if (dev_file) {
        close_joystick_device(dev_file);
        dev_file = 0;
        fprintf(stdout, "Closed joystick device\n");
        fflush(stdout);
    }
}

static volatile int timer_file = 0;

void timer_file_closer() {
    if (timer_file) {
        close(timer_file);
        timer_file = 0;
    }
}

int setup_timerfd() {
    struct itimerspec spec;
    spec.it_interval.tv_sec = 0;
    spec.it_interval.tv_nsec = UPDATE_MICROSECONDS * 1000;
    spec.it_value.tv_sec = 0;
    spec.it_value.tv_nsec = UPDATE_MICROSECONDS * 1000;

    int tfd = timerfd_create(CLOCK_MONOTONIC, 0);
    if (tfd <= 0) {
        return -1;
    }

    if (timerfd_settime(tfd, 0, &spec, NULL) < 0) {
        // In theory we should check errno; in practice idgaf (yet)
        return -1;
    }

    return tfd;
}

// Scale I2C joystick values to expected values for evdev joysticks
int16_t scale_i2c_axis_value(int16_t value) {
    int32_t scaled;
    if (value > 0) {
        scaled = (int32_t)value * JOYSTICK_AXIS_MAX / JOYSTICK_AXIS_I2C_MAX;
    } else if (value < 0) {
        scaled = (int32_t)value * JOYSTICK_AXIS_MIN / JOYSTICK_AXIS_I2C_MIN;
    } else {
        scaled = (int32_t)value;
    }
    if (scaled > JOYSTICK_AXIS_MAX) {
        return JOYSTICK_AXIS_MAX;
    }
    if (scaled < JOYSTICK_AXIS_MIN) {
        return JOYSTICK_AXIS_MIN;
    }
    return (int16_t)scaled;
}

int main(int argc, char *argv[]) {
    // I2C-side setup
    i2c_file = open_i2c_device(I2C_BUS_PATH);
    atexit(i2c_file_closer);
    fprintf(stdout, "Connected to I2C bus at %s\n", I2C_BUS_PATH);
    fflush(stdout);

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
    dev_file = setup_joystick_device(JOYSTICK_AXIS_FUZZ, JOYSTICK_AXIS_FLAT);
    if (dev_file < 0) {
        fprintf(stderr, "Couldn't set up device, exiting...\n");
        exit(1);
    }
    atexit(dev_file_closer);
    fprintf(stdout, "Created joystick device\n");
    fflush(stdout);

    // Setup timer
    timer_file = setup_timerfd();
    if (timer_file < 0) {
        fprintf(stderr, "Couldn't set up timer, exiting...\n");
        exit(1);
    }
    atexit(timer_file_closer);

    signal(SIGINT, sig_exit_handler);
    signal(SIGTERM, sig_exit_handler);

    JoystickState joystick_state = {0, 0, 0, 0, 0, 0};
    uint64_t slept_intervals;
    int exit_code = 0;

    while(running) {
        // Will block until next timer interval
        if (read(timer_file, &slept_intervals, sizeof(slept_intervals)) < 0) {
            fprintf(stderr, "Couldn't read timer file descriptor!\n");
            exit_code = 1;
            break;
        }

        if (read_i2c_joystick(i2c_file, I2C_LEFT_STICK_ADDR, &left_stick) < 0) {
            fprintf(stderr, "Couldn't read left joystick, skipping update\n");
            continue;
        }

        if (read_i2c_joystick(i2c_file, I2C_RIGHT_STICK_ADDR, &right_stick) < 0) {
            fprintf(stderr, "Couldn't read right joystick, skipping update\n");
            continue;
        }

        JoystickState new_state = {
            .l_x_axis = scale_i2c_axis_value(left_stick.x_axis),
            .l_y_axis = scale_i2c_axis_value(left_stick.y_axis),
            .r_x_axis = scale_i2c_axis_value(right_stick.x_axis),
            .r_y_axis = scale_i2c_axis_value(right_stick.y_axis),
            .l_button = left_stick.button ? 1 : 0,
            .r_button = right_stick.button ? 1 : 0,
        };
        // We don't currently actually need the updated bitmap
        if (update_joystick_state(dev_file, &joystick_state, &new_state) < 0) {
            fprintf(stderr, "Couldn't update joystick state\n");
        }
    }

    // Clear joystick colors
    if (set_i2c_joystick_color(i2c_file, I2C_LEFT_STICK_ADDR, 0) < 0) {
        exit_code = 1;
    }
    if (set_i2c_joystick_color(i2c_file, I2C_RIGHT_STICK_ADDR, 0) < 0) {
        exit_code = 1;
    }

    fprintf(stdout, "\nExiting...\n");
    fflush(stdout);
    exit(exit_code);
}
