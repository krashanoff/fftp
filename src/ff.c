#include <unistd.h>
#include <poll.h>
#include <stdio.h>
#include <strings.h>
#include <stdlib.h>
#include <errno.h>
#include <arpa/inet.h>
#include <sys/socket.h>

#include <sodium.h>

#include "common.h"

// Program options
static char buf[MAX_FRAME_SIZE];

static inline
void die()
{
  perror(strerror(errno));
  exit(EXIT_FAILURE);
}

int main(int argc, char **argv)
{
  struct sockaddr_in bound, out;

  int sockfd = 0;
  if ((sockfd = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP)) < 0)
    die();

  memset((char *)&bound, 0, sizeof(bound));
  bound.sin_family = AF_INET;
  bound.sin_port = htons(8080);
  bound.sin_addr.s_addr = htonl(INADDR_ANY);
  if (bind(sockfd, (struct sockaddr *) &bound, sizeof(bound)) < 0)
    die();
  fprintf(stderr, "Listening on 8080\n");

  // Send the request.
  Request r;
  r.type = LS;
  r.id = 0x5000;
  memcpy(&r.path, "./", 3);
  
  sendto(sockfd, NULL, 0, 0, (struct sockaddr *) &out, sizeof(out));

  // Wait for replies -- or if we timeout, send the request again.
  struct pollfd socket = {sockfd, POLLIN | POLLERR | POLLHUP, 0};
  int nfds = 0;
  while ((nfds = poll(&socket, 1, 0)) >= 0)
  {
    if (socket.revents & POLLIN)
    {
      // TODO: receive a response
      socklen_t socklen = sizeof(out);
      recvfrom(sockfd, &buf, MAX_FRAME_SIZE, 0, (struct sockaddr *) &out, &socklen);
    }
  }
  exit(EXIT_SUCCESS);
}
