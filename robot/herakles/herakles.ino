#include <Arduino.h>
#include <RF24.h>
#include <motordriver_4wd.h>

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
#define ACK_MS 50
unsigned long last_ack = 0;

#define BATT_PIN A6

// // Motor values
// char motor_dir = 'S';
// int8_t motor_left = 0;
// int8_t motor_right = 0;


void setup() {
    MOTOR.init();

    // Set battery pin as input and use external voltage ref
    analogReference(EXTERNAL);
    pinMode(BATT_PIN, INPUT);

    // Radio init
    if (!radio.begin()) {
        while (1) {}
    }
    // Set PA level low for debug
    radio.setPALevel(RF24_PA_LOW);
    radio.setDataRate(RF24_250KBPS);
    // Enable dynamic payloads and payload acks
    radio.enableDynamicPayloads();
    radio.enableAckPayload();
    // Set channel and address, reading from crazyradio tx
    radio.openReadingPipe(0, control_addr);
    radio.setChannel(channel);
    // Put radio in RX mode
    radio.startListening();
}

void loop() {
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
                // (Reserved)
                break;
            case 0xF6:
                // (Reserved)
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

    if (current_tick - last_ack >= ACK_MS) {
        // Update next ack and stage this one for sending
        do {
            last_ack += ACK_MS;
        } while (current_tick - last_ack >= ACK_MS);

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

        radio.writeAckPayload(0, ack, 3);
    }
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
