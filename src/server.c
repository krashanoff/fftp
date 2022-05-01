#include <stdlib.h>
#include <unistd.h>

#include <sys/socket.h>
#include <sys/types.h>
#include <netinet/in.h>

#include <string.h>
#include <errno.h>

#include <poll.h>

#include <stdio.h>

void die();
void *generateResponse(const char *);

int main(int argc, char **argv)
{
  int c = 0;
  while ((c = getopt(argc, argv, "46vdC:p:")) > 0)
  {
    switch (c)
    {
    case '4':
    case '6':
    case 'v':
    case 'd':
      printf("hmm\n");
      break;
    case 'C':
      printf("set directory to %s\n", optarg);
      break;
    default:
      printf("Unknown\n");
      break;
    }
  }

  // Initialize socket.
  int fd = socket(AF_INET, SOCK_DGRAM, 0);

  struct sockaddr_in addr;
  addr.sin_family = AF_INET;
  addr.sin_addr.s_addr = INADDR_ANY;
  addr.sin_port = htons(8080);
  socklen_t addrlen = sizeof(addr);
  bind(fd, (struct sockaddr *)&addr, sizeof(addr));

  // Poll for input, send responses.
  char recvbuf[4096] = {0};
  ssize_t recvd = 0;
  struct pollfd fds[] = {
      {fd, POLLIN | POLLHUP | POLLERR, 0},
  };
  while (poll(fds, 1, -1) >= 0)
  {
    if (fds[0].revents & POLLIN)
    {
      // TODO: handle if the receive fails.
      if ((recvd = recvfrom(fd, recvbuf, sizeof(recvbuf), 0, (struct sockaddr *)&addr, &addrlen)) < 0)
        return 1;

      uint8_t meta = recvbuf[0];
      char *data = recvbuf + 1;
      printf("Packet len is %d, meta is %d, data is %s\n", len, meta, data);

      printf("Reply to %d\n", addr.sin_port);
      memcpy(recvbuf, "STOP", 5);
      fprintf(stderr, "Sent response %d.\n", sendto(fd, recvbuf, 5, 0, (struct sockaddr *)&addr, &addrlen));
    }
  }
  return 0;
}

void die()
{
  fprintf(stderr, "%s\n", strerror(errno));
  exit(EXIT_FAILURE);
}
