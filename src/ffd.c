#include <unistd.h>
#include <stdio.h>
#include <stdlib.h>

#include <getopt.h>
#include <strings.h>
#include <errno.h>

#include <poll.h>
#include <arpa/inet.h>
#include <sys/socket.h>

#include <sodium.h>

#include "common.h"

static struct option options[] = {
  {0, 0, 0, 0}
};

char buf[MAX_FRAME_SIZE];
int domain = 0;
uint32_t port = -1;

static inline
void die()
{
  perror(strerror(errno));
  exit(EXIT_FAILURE);
}

void usage(char **argv) {
  fprintf(stderr, "Usage: %s [-46d] [-p PORT]\n", argv[0]);
}

void parseArgs(int argc, char **argv) {
  int idx = 0;
  char c;
  while ((c = getopt_long(argc, argv, "46dp:", options, &idx)) != -1) {
    switch (c) {
    case 0:
      fprintf(stderr, "Got option %s\n", options[idx].name);
      break;
    case '4':
      domain = AF_INET;
      break;
    case '6':
      domain = AF_INET6;
      break;
    case 'p':
      port = htons(optarg);
      break;
    case '?':
      break;
    default:
      fprintf(stderr, "Got an unknown character\n");
    }
  }

  if (optind < argc) {
    while (optind < argc) {
      fprintf(stderr, "%s\n", argv[optind++]);
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
  bound.sin_port = port;
  bound.sin_addr.s_addr = htonl(INADDR_ANY);
  if (bind(sockfd, (struct sockaddr *) &bound, (socklen_t) sizeof(bound)) < 0)
    die();
  fprintf(stderr, "Bound to port %d.\n", 8080);

  printf("Testing something...\n");
  Frame f;
  buildFrame("test", 5, &f);
  printf("Created frame...\n");

  Frame f2;
  parseFrame(&f, sizeof(f), &f2);

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
