.PHONY: ff ffd
ff:
	$(CC) -o $@ -O2 ff.c

ffd:
	$(CC) -o $@ -O2 ffd.c
