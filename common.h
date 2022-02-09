#ifndef COMMON_H_
#define COMMON_H_

/*
Common functionality and data types for FFTP.
*/

#include <stdlib.h>
#include <string.h>

#include <arpa/inet.h>
#include <sys/socket.h>

#include "sodium/include/sodium.h"

#define LS 1
#define DL 1 << 1

#define HEADER_SIZE 12
#define MAX_FRAME_SIZE 65536

#define FRAME_INITIATE  ((uint8_t) 1     )
#define FRAME_FIRST     ((uint8_t) 1 << 1)
#define FRAME_CONNECTED ((uint8_t) 1 << 2)

struct header_t
{
  uint16_t len;
  uint8_t frameType;
  char checksum[crypto_generichash_BYTES];
};
typedef struct header_t Header;

struct frame_t
{
  Header header;

  // Data contained within the frame.
  void *data;
};
typedef struct frame_t Frame;

// Buffer should be NULL-terminated.
Frame *deserializeFrame(const char const *buf, size_t len) {
  if (len < sizeof(Header))
    return NULL;

  char *cursor = buf;
  Frame output;
  memcpy(&output, cursor, sizeof(Header));
  cursor += sizeof(Header);

  fprintf(stderr, "Header claims packet is %d bytes long\n", output.header.len);
  return NULL;
}

void freeFrame(Frame *f) {
  free(f->data);
  free(f);
}

struct request_t
{
  // Type of the request.
  uint8_t type;

  // Path of concern.
  char path[2048];

  // Request ID.
  uint8_t id;
};
typedef struct request_t Request;

typedef struct Response
{
  char data[MAX_FRAME_SIZE];
} Response;

#endif // COMMON_H_