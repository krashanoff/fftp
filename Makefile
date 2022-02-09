CC=gcc
CFLAGS=-std=c11 -O2
LDFLAGS=-Isodium/include
WFLAGS=-Wall -Wextra

ff: src/ff.o
	$(CC) $(CFLAGS) $(WFLAGS) $(LDFLAGS) -o $@ $^ sodium/lib/libsodium.a

ffd: src/ffd.o
	$(CC) $(CFLAGS) $(WFLAGS) $(LDFLAGS) -o $@ $^ sodium/lib/libsodium.a

%.o: %.c
	$(CC) $(CFLAGS) $(WFLAGS) $(LDFLAGS) -c -o $@ $^

.PHONY: clean
clean:
	rm -rf src/*.o ff ffd
