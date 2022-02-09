#include <unistd.h>
#include <stdio.h>
#include <stdlib.h>

#include <getopt.h>
#include <strings.h>
#include <errno.h>

#include <poll.h>
#include <arpa/inet.h>
#include <sys/socket.h>

#include "common.h"
#include "sodium/include/sodium.h"

static struct option options[] = {
  {0, 0, 0, 0}
};

char buf[MAX_FRAME_SIZE];
uint32_t port = -1;

static inline
void die()
{
  perror(strerror(errno));
  exit(EXIT_FAILURE);
}

void usage(char **argv) {
  fprintf(stderr, "Usage: %s [-46]\n", argv[0]);
}

void parseArgs(int argc, char **argv) {
  int optidx = 0;
  char c;
  while ((c = getopt_long(argc, argv, "46", options, &optidx)) != -1) {
    switch (c) {
    case 0:
      fprintf(stderr, "Got option %s\n", options[optidx].name);
      break;
    case '4':
      break;
    case '6':
      break;
    case '?':
      break;
    default:
      fprintf(stderr, "Got an unknown character\n");
    }
  }
  
  if (optidx < argc) {
    while (optidx < argc) {
      fprintf(stderr, "%s\n", argv[optidx++]);
    }
  }
}

int main(int argc, char **argv)
{
  parseArgs(argc, argv);

  if (sodium_init() < 0)
    die();

  int sockfd = 0;
  if ((sockfd = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP)) < 0)
    die();

  struct sockaddr_in bound, out;
  sodium_memzero(&bound, sizeof(bound));
  bound.sin_family = AF_INET;
  bound.sin_port = htons(8080);
  bound.sin_addr.s_addr = htonl(INADDR_ANY);
  if (bind(sockfd, (struct sockaddr *) &bound, (socklen_t) sizeof(bound)) < 0)
    die();
  fprintf(stderr, "Bound to port %d.\n", 8080);

  struct pollfd socket = {sockfd, POLLOUT | POLLIN | POLLERR | POLLHUP, 0};
  int nfds = 0;
  while ((nfds = poll(&socket, 1, 0)) >= 0)
  {
    if (socket.revents & (POLLERR | POLLHUP))
      die();

    if (socket.revents & POLLIN)
    {
      // TODO: receive a request
      socklen_t outlen = sizeof(out);
      recvfrom(sockfd, &buf, MAX_FRAME_SIZE, 0, (struct sockaddr *) &out, &outlen);
    }
    if (socket.revents & POLLOUT)
    {
      // printf("Send something\n");
    }
  }
  exit(EXIT_SUCCESS);
}
