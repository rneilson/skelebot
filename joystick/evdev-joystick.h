#ifndef EVDEV_JOYSTICK_H
#define EVDEV_JOYSTICK_H

#include <stdint.h>

typedef struct {
    int16_t l_x_axis;
    int16_t l_y_axis;
    int16_t r_x_axis;
    int16_t r_y_axis;
    uint8_t l_button;
    uint8_t r_button;
} JoystickState;

static int setup_axis(int file, unsigned axis, int min, int max);

// Creates and initializes a new joystick device with provided `min` and `max`
// values. Returns file descriptor on success, or -1 on error.
int setup_joystick_device(int min, int max);

// Updates `state` with any changed values from `new_state`, and emits the
// corresponding events to the joystick device given by `file`. Returns a bit
// vector of which axes/buttons were updated, 0 if no changes, or -1 on error.
int update_joystick_state(int file, JoystickState *state, JoystickState *new_state);

// Destroys input device and closes file.
int close_joystick_device(int file);

#endif // EVDEV_JOYSTICK_H
