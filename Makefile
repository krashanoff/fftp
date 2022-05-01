CC=gcc
OPT=-O2
CFLAGS=-std=c99
WFLAGS=-Wall -Wextra

default: server client

server: server.o
	$(CC) $(CFLAGS) $(OPT) $(WFLAGS) -o ffd $^

client: client.o
	$(CC) $(CFLAGS) $(OPT) $(WFLAGS) -o ff $^

%.o: src/%.c
	$(CC) $(CFLAGS) $(OPT) $(WFLAGS) -c $^

.PHONY: clean
clean:
	rm -f server.o client.o ffd ff
	