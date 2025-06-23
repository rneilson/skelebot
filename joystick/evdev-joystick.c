// Some code borrowed from
// https://github.com/GrantEdwards/uinput-joystick-demo/blob/master/uinput-demo.c

#include "evdev-joystick.h"

#include <linux/input-event-codes.h>
#include <linux/input.h>
#include <linux/uinput.h>
#include <sys/ioctl.h>
#include <fcntl.h>
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

static int setup_axis(int file, unsigned axis, int min, int max) {
    if (ioctl(file, UI_SET_ABSBIT, axis) < 0) {
        fprintf(stderr, "Error in UI_SET_ABSBIT for axis 0x%x: %s\n", axis, strerror(errno));
        return -1;
    }

    struct uinput_abs_setup setup = {
        .code = axis,
        .absinfo = {
            .minimum = min,
            .maximum = max,
        },
    };

    if (ioctl(file, UI_ABS_SETUP, &setup) < 0) {
        fprintf(stderr, "Error in UI_ABS_SETUP for axis 0x%x: %s\n", axis, strerror(errno));
        return -1;
    }

    return 0;
}

int setup_joystick_device(int min, int max) {
    int file = open("/dev/uinput", O_WRONLY | O_NONBLOCK);
    if (file < 0) {
        fprintf(stderr, "Error opening /dev/uinput: %s\n", strerror(errno));
        return -1;
    }

    int res = 0;

    // Absolute position handling
    res |= ioctl(file, UI_SET_EVBIT, EV_ABS);
    res |= setup_axis(file, ABS_X, min, max);
    res |= setup_axis(file, ABS_Y, min, max);
    res |= setup_axis(file, ABS_RX, min, max);
    res |= setup_axis(file, ABS_RY, min, max);
    if (res) {
        fprintf(stderr, "One or more errors setting up joystick axes\n");
        return -1;
    }

    // Button handling
    res |= ioctl(file, UI_SET_EVBIT, EV_KEY);
    res |= ioctl(file, UI_SET_KEYBIT, BTN_THUMBL);
    res |= ioctl(file, UI_SET_KEYBIT, BTN_THUMBR);
    if (res) {
        fprintf(stderr, "One or more errors setting up joystick buttons\n");
        return -1;
    }

    // Device itself
    struct uinput_setup setup = {
        .name = "Userspace joystick device",
        .id = {
            .bustype = BUS_USB,
            .vendor = 0x0021,
            .product = 0x0021,
            .version = 1,
        },
    };
    if (ioctl(file, UI_DEV_SETUP, &setup) < 0) {
        fprintf(stderr, "Error setting up joystick device: %s\n", strerror(errno));
        return -1;
    }
    if (ioctl(file, UI_DEV_CREATE) < 0) {
        fprintf(stderr, "Error creating joystick device: %s\n", strerror(errno));
        return -1;
    }

    return file;
}

int update_joystick_state(int file, JoystickState *state, JoystickState *new_state) {
    int updated = 0;

    int events = 0;
    struct input_event event_list[7];   // 4 axes, 2 buttons, and a sync
    memset(&event_list, 0, sizeof(event_list));

    if (state->l_x_axis != new_state->l_x_axis) {
        event_list[events].type = EV_ABS;
        event_list[events].code = ABS_X;
        event_list[events].value = new_state->l_x_axis;
        events++;
        updated |= (1 << 0);
    }
    if (state->l_y_axis != new_state->l_y_axis) {
        event_list[events].type = EV_ABS;
        event_list[events].code = ABS_Y;
        event_list[events].value = new_state->l_y_axis;
        events++;
        updated |= (1 << 1);
    }
    if (state->r_x_axis != new_state->r_x_axis) {
        event_list[events].type = EV_ABS;
        event_list[events].code = ABS_RX;
        event_list[events].value = new_state->r_x_axis;
        events++;
        updated |= (1 << 2);
    }
    if (state->r_y_axis != new_state->r_y_axis) {
        event_list[events].type = EV_ABS;
        event_list[events].code = ABS_RY;
        event_list[events].value = new_state->r_y_axis;
        events++;
        updated |= (1 << 3);
    }
    if (state->l_button != new_state->l_button) {
        event_list[events].type = EV_KEY;
        event_list[events].code = BTN_THUMBL;
        event_list[events].value = new_state->l_button;
        events++;
        updated |= (1 << 4);
    }
    if (state->r_button != new_state->r_button) {
        event_list[events].type = EV_KEY;
        event_list[events].code = BTN_THUMBR;
        event_list[events].value = new_state->r_button;
        events++;
        updated |= (1 << 5);
    }

    // Sync event if required
    if (events) {
        event_list[events].type = EV_SYN;
        event_list[events].code = SYN_REPORT;
        event_list[events].value = 0;
        events++;
        if (write(file, &event_list, sizeof(event_list[0]) * events) < 0) {
            fprintf(stderr, "Error writing %d joystick events: %s\n", events, strerror(errno));
            return -1;
        }
    }

    if (updated) {
        *state = *new_state;
    }

    return updated;
}

int close_joystick_device(int file) {
    if (ioctl(file, UI_DEV_DESTROY) < 0) {
        fprintf(stderr, "Error destroying joystick device: %s\n", strerror(errno));
        return -1;
    }
    close(file);
    return 0;
}
