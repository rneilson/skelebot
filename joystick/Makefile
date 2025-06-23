joystick-test: joystick-test.c i2c-joystick.o
	gcc -static -o joystick-test joystick-test.c i2c-joystick.o -li2c

clean:
	rm *.o
	rm -f joystick-test
