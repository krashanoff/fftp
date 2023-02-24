#ifndef SHARED_H_
#define SHARED_H_

#include <stdlib.h>

#define REQUEST_LS      ('L')     /* list a whole directory */
#define REQUEST_ENTRY   ('E')     /* list one part of the directory */
#define REQUEST_CHUNK   ('C')     /* get a chunk of a file */
#define REQUEST_GET     ('G')     /* get the whole file */

#define RESPONSE_LS     ('d')     /* entry in a directory */
#define RESPONSE_CHUNK  ('c')     /* chunk in a file */
#define RESPONSE_TERM   ('t')     /* terminal chunk in a file */

struct request_t {
  uint8_t tag;
  uint32_t requested_buffer_size;
  const char *data;
};
typedef struct request_t Request;

struct response_t {
  uint32_t length;
  uint8_t tag;
  const char *data;
};
typedef struct response_t Response;

/**
 * NULL for error, caller free.
 */
Request parse_request(const char *data, int len, int local_buffer_cap) {
  uint32_t packet_len = data[0];
  uint8_t tag = data[1];
  uint32_t requested_buffer_size = (
    (data[2] << 24) |
    (data[3] << 16) |
    (data[4] << 8) |
    (data[5])
  );
  
  Request result;
  result.tag = tag;
  result.requested_buffer_size = requested_buffer_size;
  result.data = data + 6;
  return result;
}

#endif // SHARED_H_
