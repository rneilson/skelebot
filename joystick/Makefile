joystick-daemon: joystick-daemon.c i2c-joystick.o evdev-joystick.o
	gcc -static -o joystick-daemon joystick-daemon.c i2c-joystick.o evdev-joystick.o -li2c

joystick-test: joystick-test.c i2c-joystick.o
	gcc -static -o joystick-test joystick-test.c i2c-joystick.o -li2c

clean:
	rm *.o
	rm -f joystick-test
	rm -f joystick-daemon
