CC=gcc
CFLAGS=-std=c11 -O3
LDFLAGS=-Isodium/include
WFLAGS=-Wall -Wextra

ff: ff.o
	$(CC) $(CFLAGS) $(WFLAGS) $(LDFLAGS) -o $@ $^ sodium/lib/libsodium.a

ffd: ffd.o
	$(CC) $(CFLAGS) $(WFLAGS) $(LDFLAGS) -o $@ $^ sodium/lib/libsodium.a

%.o: %.c
	$(CC) $(CFLAGS) $(WFLAGS) $(LDFLAGS) -c -o $@ $^

.PHONY: clean
clean:
	rm -rf *.o ff ffd
