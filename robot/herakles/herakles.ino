#include <motordriver_4wd.h>
#include <RF24.h>
#include <Wire.h>
#include <Arduino.h>

#define CE_PIN A0
#define CSN_PIN A1

RF24 radio(CE_PIN, CSN_PIN);

// Command buffer
uint8_t commands[4][4];
uint8_t command_head = 0;
uint8_t command_tail = 0;

// Radio params
uint8_t channel = 76;   // Default for RF24 lib, Crazyradio needs changing
uint8_t control_addr[6] = {0xE7, 0xE7, 0xE7, 0xE7, 0xE7, 0x00}; // Default for Crazyradio

// Timer values
#define ACK_PAYLOAD_MS 50
unsigned long last_ack = 0;
#define CONN_LOSS_MS 200
unsigned long last_cmd = 0;

#define MAIN_BATT_PIN A6
#define AUX_BATT_PIN A2
#define AUX_EN_PIN 2

// // Motor values
// char motor_dir = 'S';
// int8_t motor_left = 0;
// int8_t motor_right = 0;

// Servo values
#define SERVO_I2C_ADDR 0x24
#define SERVO_PWM_FREQ_REG 0x60
#define SERVO_PWM_FREQ_VAL 50   // 50 Hz
#define SERVO_PAN_PIN_REG 0x02
#define SERVO_TILT_PIN_REG 0x03
#define SERVO_PIN_PWM_MODE 0x20
#define SERVO_PAN_PWM_REG 0x52
#define SERVO_TILT_PWM_REG 0x54
#define SERVO_PAN_MIN 87    // 0°
#define SERVO_PAN_MID 305   // 90°
#define SERVO_PAN_MAX 499   // 180°
// #define SERVO_TILT_MIN 87    // 0°
#define SERVO_TILT_MIN 195   // 45°
#define SERVO_TILT_MID 305   // 90°
#define SERVO_TILT_MAX 394   // 135°
// #define SERVO_TILT_MAX 499   // 180°

void setup() {
    MOTOR.init();

    // Set battery pins as input and use external voltage ref
    analogReference(EXTERNAL);
    pinMode(MAIN_BATT_PIN, INPUT);
    // pinMode(AUX_BATT_PIN, INPUT);

    // TODO: set aux pin low to start
    // pinMode(AUX_EN_PIN, OUTPUT);
    // digitalWrite(AUX_EN_PIN, LOW);

    // Setup I2C for servo control
    Wire.begin();
    writeI2CRegisterWord(SERVO_PWM_FREQ_REG, SERVO_PWM_FREQ_VAL);
    writeI2CRegisterWord(SERVO_PAN_PWM_REG, SERVO_PAN_MID);
    writeI2CRegisterWord(SERVO_TILT_PWM_REG, SERVO_TILT_MID);
    writeI2CRegisterByte(SERVO_PAN_PIN_REG, SERVO_PIN_PWM_MODE);
    writeI2CRegisterByte(SERVO_TILT_PIN_REG, SERVO_PIN_PWM_MODE);

    // Initial servo wakeup stretching
    delay(400);
    setCameraPanAngle(0);
    delay(400);
    setCameraPanAngle(90);
    delay(400);
    setCameraPanAngle(180);
    delay(400);
    setCameraPanAngle(90);
    delay(400);
    setCameraTiltAngle(0);
    delay(400);
    setCameraTiltAngle(90);
    delay(400);
    setCameraTiltAngle(180);
    delay(400);
    setCameraTiltAngle(90);
    delay(400);

    // Radio init
    if (!radio.begin()) {
        while (1) {}
    }
    // Set PA level high for use
    // TODO: figure out if/when to set to max
    radio.setPALevel(RF24_PA_HIGH);
    radio.setDataRate(RF24_250KBPS);
    // Enable dynamic payloads and payload acks
    radio.enableDynamicPayloads();
    radio.enableAckPayload();
    // Set channel and address, reading from crazyradio tx
    radio.openReadingPipe(0, control_addr);
    radio.setChannel(channel);
    // Put radio in RX mode
    radio.startListening();

    // TODO: now enable aux
    // digitalWrite(AUX_EN_PIN, HIGH);
}

void loop() {
    bool received_cmd = false;

    // First, handle any payload we've received
    // Only one payload received per loop for now, may refactor later
    if (radio.available()) {
        uint8_t command_len = radio.getDynamicPayloadSize();
        // Really should have an error flag somewhere around here
        // We shouldn't ever get more than 4 bytes as per protocol
        if (command_len > 4) {
            command_len = 4;
        }
        uint8_t next_head = (command_head + 1) % 4;
        // We have to drop payloads if the ring buffer's full
        if (next_head != command_tail) {
            radio.read(commands[next_head], command_len);
            command_head = next_head;
        }
        received_cmd = true;
    }

    // Potentially process multiple payloads, however
    while (command_tail != command_head) {
        // TODO: move this all to a function
        uint8_t (&command)[4] = commands[command_tail];
        switch (command[0]) {
            case 0xF0:
                // No-op
                break;
            case 0xF1:
                // Change channel
                radio.setChannel(command[1]);
                break;
            case 0xF2:
                // (Reserved)
                break;
            case 0xF3:
                // Stop
                MOTOR.setStop1();
                MOTOR.setStop2();
                break;
            case 0xF4:
                // Drive (L, R)
                setLeftMotorSpeed(command[1]);
                setRightMotorSpeed(command[2]);
                break;
            case 0xF5:
                // Center camera
                setCameraPanAngle(90);
                setCameraTiltAngle(90);
                break;
                case 0xF6:
                // Look (pan, tilt)
                setCameraPanAngle(command[1]);
                setCameraTiltAngle(command[2]);
                break;
            case 0xF7:
                // (Reserved)
                break;
            default:
                break;
        }
        command_tail = (command_tail + 1) % 4;
    }

    unsigned long current_tick = millis();

    // If no command received in CONN_LOSS_MS ms, assume connection lost and stop
    if (received_cmd) {
        last_cmd = current_tick;
    }
    if (current_tick - last_cmd >= CONN_LOSS_MS) {
        MOTOR.setStop1();
        MOTOR.setStop2();
        // This will
        last_cmd = current_tick;
    }

    if (current_tick - last_ack >= ACK_PAYLOAD_MS) {
        // Update next ack and stage this one for sending
        do {
            last_ack += ACK_PAYLOAD_MS;
        } while (current_tick - last_ack >= ACK_PAYLOAD_MS);

        // Get divided battery voltage from ADC6
        // R1 = 430k, R2 = 100k, Vo = Vi * (430000 + 100000) / 100000
        // We have clearance here in terms of int sizes because the
        // analog value is 10 bits
        uint16_t voltage = getBatteryVoltage();
        uint8_t *voltage_bytes = (uint8_t*)&voltage;

        uint8_t ack[3];
        ack[0] = 0xFB;  // Battery voltage
        // Convert little-endian to big-endian
        ack[1] = voltage_bytes[1];
        ack[2] = voltage_bytes[0];

        // TODO: queue up ack payloads
        radio.writeAckPayload(0, ack, 3);
    }

    // TODO: write ack payload(s) and drain queue if successful
}

bool writeI2CRegisterByte(uint8_t reg, uint8_t value) {
    Wire.beginTransmission(SERVO_I2C_ADDR);
    Wire.write(reg);
    Wire.write(value);
    return Wire.endTransmission() == 0;
}

bool writeI2CRegisterWord(uint8_t reg, uint16_t value) {
    Wire.beginTransmission(SERVO_I2C_ADDR);
    Wire.write(reg);
    Wire.write((uint8_t *)&value, sizeof(value));
    return Wire.endTransmission() == 0;
}

uint16_t mapCameraAngle(uint16_t angle, uint16_t in_min, uint16_t in_max, uint16_t out_min, uint16_t out_max) {
    return (angle - in_min) * (out_max - out_min) / (in_max - in_min) + out_min;
}

bool setCameraAngle(uint8_t reg, uint8_t angle, uint16_t min, uint16_t mid, uint16_t max) {
    uint16_t value;

    if (angle > 180) {
        angle = 180;
    }
    // Invert because both servos turn backwards
    angle = 180 - angle;

    // Slightly complex mapping because the midpoint of the servo range isn't strictly the
    // arithmetic mean of min and max...
    if (angle == 0) {
        value = min;
    } else if (angle < 90) {
        value = mapCameraAngle(angle, 0, 90, min, mid);
    } else if (angle == 90) {
        value = mid;
    } else if (angle < 180) {
        value = mapCameraAngle(angle, 90, 180, mid, max);
    } else {
        value = max;
    }

    // Now write the change
    return writeI2CRegisterWord(reg, value);
}

bool setCameraPanAngle(uint8_t angle) {
    return setCameraAngle(SERVO_PAN_PWM_REG, angle, SERVO_PAN_MIN, SERVO_PAN_MID, SERVO_PAN_MAX);
}

bool setCameraTiltAngle(uint8_t angle) {
    return setCameraAngle(SERVO_TILT_PWM_REG, angle, SERVO_TILT_MIN, SERVO_TILT_MID, SERVO_TILT_MAX);
}

uint8_t motorSpeedCeiling(uint8_t value) {
    return value < 100 ? value : 100;
}

void setLeftMotorSpeed(uint8_t value) {
    uint8_t speed;
    uint8_t dir;

    if (value >= 100) {
        speed = motorSpeedCeiling(value - 100);
        dir = DIRF;
    } else {
        speed = 100 - value;
        dir = DIRR;
    }

    MOTOR.setSpeedDir1(speed, dir);
}

void setRightMotorSpeed(uint8_t value) {
    uint8_t speed;
    uint8_t dir;

    if (value >= 100) {
        speed = motorSpeedCeiling(value - 100);
        dir = DIRR; // Right reversed
    } else {
        speed = 100 - value;
        dir = DIRF; // Right reversed
    }

    MOTOR.setSpeedDir2(speed, dir);
}

uint16_t getBatteryVoltage() {
    uint16_t voltage = 0;

    // Average over 4 readings
    for (char i = 0; i < 4; i++) {
        voltage += analogRead(A6);
    }
    voltage = voltage / 4;

    // Get divided voltage
    // R1 = 430k, R2 = 100k, Vo = Vi * (430000 + 100000) / 100000
    // Then multiply by 5, as the reference is 5V
    // ((430000 + 100000) / 100000) * 5 = 530 * 5 / 100 = 53 / 2
    // We have clearance here in terms of int sizes because the
    // analog value is 10 bits
    voltage = voltage * 53 / 2;

    return voltage;
}
