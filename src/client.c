#include <stdlib.h>
#include <unistd.h>

#include <sys/socket.h>
#include <sys/types.h>
#include <netinet/in.h>

#include <string.h>
#include <errno.h>

#include <poll.h>

#include <stdio.h>

uint16_t port = -1;

int main(int argc, char **argv)
{
  int c = 0;
  while ((c = getopt(argc, argv, "46p:")) > 0)
  {
    switch (c)
    {
    case '4':
    case '6':
      break;
    case 'p':
      if ((port = atoi(optarg)) < 0)
      {
        perror(strerror(errno));
        exit(EXIT_FAILURE);
      }
      break;
    default:
      break;
    }
  }

  int fd = socket(AF_INET, SOCK_DGRAM, 0);
  struct sockaddr_in addr;
  addr.sin_family = AF_INET;
  addr.sin_addr.s_addr = INADDR_ANY;
  addr.sin_port = htons(port);
  socklen_t addrlen = sizeof(addr);
  bind(fd, (struct sockaddr *)&addr, sizeof(addr));

  char sendbuf[4096];
  ssize_t recvd = 0;
  struct pollfd fds[] = {
      {fd, POLLIN | POLLOUT | POLLHUP | POLLERR, 0}};
  while (poll(fds, 1, -1) >= 0)
  {
    // Once our request is received, we can disable the necessity
    // to send messages out.
    if (fds[0].revents & POLLIN)
    {
      fprintf(stderr, "Received a response\n");
      fds[0].events &= ~POLLOUT;
    }

    if (fds[0].revents & POLLOUT)
    {
      memcpy(sendbuf, "testing", 8);
      recvd = sendto(fd, sendbuf, 8, 0, (struct sockaddr *)&addr, addrlen);
      printf("Thing %zd on port %d\n", recvd, addr.sin_port);
    }
  }

  return 0;
}