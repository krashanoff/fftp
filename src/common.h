#ifndef COMMON_H_
#define COMMON_H_

/*
Common functionality and data types for FFTP.
*/

#include <stdlib.h>
#include <string.h>

#include <arpa/inet.h>
#include <sys/socket.h>

#include <sodium.h>

#define LS 1
#define DL 1 << 1

#define HEADER_SIZE 12
#define MAX_FRAME_SIZE 65536

#define FRAME_INITIATE ((uint8_t)1)
#define FRAME_FIRST ((uint8_t)1 << 1)
#define FRAME_CONNECTED ((uint8_t)1 << 2)

struct header_t
{
  // Length of the data field.
  uint16_t len;

  // Type of frame sent/received.
  uint8_t frameType;

  // Checksum over the entire packet, computed when
  // the checksum field is zero.
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

// Must free returned value with free().
char *computeChecksum(Frame *f)
{
  char *computed = malloc(crypto_generichash_BYTES * sizeof(char));

  char oldChecksum[crypto_generichash_BYTES];
  memcpy(oldChecksum, &f->header.checksum, crypto_generichash_BYTES * sizeof(char));
  memset(&f->header.checksum, 0, crypto_generichash_BYTES * sizeof(char));

  size_t bufLen = sizeof(Header) + f->header.len;
  char *buf = malloc(bufLen);
  memcpy(buf, &f->header, sizeof(Header));
  memcpy(buf + sizeof(Header), f->data, f->header.len);
  crypto_generichash(computed, crypto_generichash_BYTES * sizeof(char), buf, bufLen, NULL, 0);
  memcpy(&f->header.checksum, oldChecksum, crypto_generichash_BYTES * sizeof(char));

  return computed;
}

// Build a frame containing a buffer of bytes.
int buildFrame(const void *data, size_t len, Frame *out)
{
  out->header.len = len;
  out->header.frameType = FRAME_CONNECTED;
  out->data = data;

  char *checksum = computeChecksum(out);
  memcpy(&out->header.checksum, checksum, crypto_generichash_BYTES * sizeof(char));
  free(checksum);
  return 0;
}

// Deserialize a Frame from some bytes.
// `len` is the length of the entire packet read.
// Returns -1 if the buffer cannot support a Frame, or is ill-formatted.
int parseFrame(const char *buf, size_t len, Frame *out)
{
  if (len < sizeof(Header))
    return -1;
  char *cursor = buf;
  memcpy(&out, cursor, sizeof(Header));
  cursor += sizeof(Header);
  return 0;
}

void freeFrame(Frame *f)
{
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