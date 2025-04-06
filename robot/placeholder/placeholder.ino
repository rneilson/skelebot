#include <Arduino.h>
#include <RF24.h>
#include <printf.h>

#define CE_PIN 7
#define CSN_PIN 8

RF24 radio(CE_PIN, CSN_PIN);

// Command buffer
uint8_t commands[4][4];
uint8_t command_head = 0;
uint8_t command_tail = 0;

// Radio params
uint8_t channel = 76;   // Default for RF24 lib, Crazyradio needs changing
uint8_t control_addr[6] = {0xE7, 0xE7, 0xE7, 0xE7, 0xE7, 0x00}; // Default for Crazyradio

// Timer values
#define TICK_MS 20
#define ACK_MS 50
unsigned long last_tick = 0;
unsigned long last_ack = 0;

// Motor values
char motor_dir = 'S';
int8_t motor_left = 0;
int8_t motor_right = 0;


void setup() {
    // Serial setup first
    Serial.begin(115200);
    while (!Serial) {
        // Wait for USB serial to init
    }

    // Radio init
    if (!radio.begin()) {
        Serial.println(F("Radio not responding, aborting setup..."));
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

    // Debug prints
    Serial.print(F("Listening on channel: "));
    Serial.println((int)channel);
    Serial.print(F("Listening to address: 0x"));
    for (uint8_t i = 0; i < 5; i++) {
        uint8_t b = control_addr[i];
        Serial.print(b >> 4, HEX);
        Serial.print(b & 0x0F, HEX);
    }
    Serial.print("\n");
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
                motor_dir = 'S';
                motor_left = 0;
                motor_right = 0;
                break;
            case 0xF4:
                // Forward (L+, R+)
                motor_dir = 'F';
                motor_left = motorSpeedCeiling(command[1]);
                motor_right = motorSpeedCeiling(command[2]);
                break;
            case 0xF5:
                // Turn right (L+, R-)
                motor_dir = 'R';
                motor_left = motorSpeedCeiling(command[1]);
                motor_right = 0 - motorSpeedCeiling(command[2]);
                break;
            case 0xF6:
                // Turn left (L-, R+)
                motor_dir = 'L';
                motor_left = 0 - motorSpeedCeiling(command[1]);
                motor_right = motorSpeedCeiling(command[2]);
                break;
            case 0xF7:
                // Backward (L-, R-)
                motor_dir = 'B';
                motor_left = 0 - motorSpeedCeiling(command[1]);
                motor_right = 0 - motorSpeedCeiling(command[2]);
                break;
            default:
                break;
        }
        command_tail = (command_tail + 1) % 4;
    }

    unsigned long current_tick = millis();

    if (current_tick - last_tick >= TICK_MS) {
        // Update next tick and execute this one
        do {
            last_tick = current_tick + TICK_MS;
        } while (current_tick - last_tick >= TICK_MS);

        // Output current drive values to serial
        char output_str[16];
        sprintf(output_str, "\r%c %+.3i %+.3i", motor_dir, motor_left, motor_right);
        Serial.print(output_str);
    }

    if (current_tick - last_ack >= ACK_MS) {
        // Update next ack and stage this one for sending
        do {
            last_ack = current_tick + ACK_MS;
        } while (current_tick - last_ack >= ACK_MS);

        // Fake a battery voltage using the current tick
        uint16_t voltage = (uint16_t)65535 - (uint16_t)(current_tick & 0xFFFF);
        uint8_t *voltage_bytes = (uint8_t*)&voltage;

        uint8_t ack[3];
        ack[0] = 0xFB;  // Battery voltage
        // Convert little-endian to big-endian
        ack[1] = voltage_bytes[1];
        ack[2] = voltage_bytes[0];

        radio.writeAckPayload(0, ack, 3);
    }
}

int8_t motorSpeedCeiling(uint8_t value) {
    return (int8_t)(value < 100 ? value : 100);
}
