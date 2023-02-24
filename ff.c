#include <stdlib.h>
#include <unistd.h>
#include <string.h>
#include <stdio.h>
#include <poll.h>

#include <signal.h>

#include <netdb.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <arpa/inet.h>

#include "proto.h"

#define MODE_LS (1)       /* ask to list files */
#define MODE_GET (1 << 1) /* ask to get files */

int fd = -1;
char recv_buf[1000 * 1000] = {0}; // 1M is more than enough

void die(int, const char *);

int main(int argc, char *const *argv)
{
  if (argc < 3)
  {
    die(EXIT_FAILURE, "usage: [address] [port] [ls|get] [PATH...]");
  }

  fprintf(stderr, "addr %s\n", argv[1]);
  fprintf(stderr, "mode %s\n", argv[3]);
  int mode = strcasecmp("get", argv[3]) == 0
                 ? MODE_GET
             : strcasecmp("ls", argv[3]) == 0
                 ? MODE_LS
                 : -1;
  if (mode < 0)
  {
    die(EXIT_FAILURE, "usage: [ls|get] PATH...");
  }

  struct sockaddr_in saddr = {0};
  saddr.sin_family = AF_INET;
  saddr.sin_port = htons(0);
  if (inet_aton(argv[1], &saddr.sin_addr) < 0)
  {
    die(EXIT_FAILURE, "invalid address");
  }

  if ((fd = socket(AF_INET, SOCK_DGRAM, 0)) < 0)
  {
    die(EXIT_FAILURE, "failed to open socket");
  }

  if (bind(fd, &saddr, sizeof(saddr)) < 0)
  {
    die(EXIT_FAILURE, "failed to bind socket");
  }

  int server_port = atoi(argv[2]);
  if (server_port < 0)
    die(EXIT_FAILURE, "invalid port");
  saddr.sin_port = htons(server_port);

  // Send initiation packets for each path
  for (int k = 4; k < argc; k++)
  {
    recv_buf[0] = 0xD0;
    recv_buf[1] = 0xDF;
    recv_buf[2] = (4 << 4) | 6;
    recv_buf[3] = '\0';

    fprintf(stderr, "sending packet with len %x, type %d, tag %d\n", 0xD0DF, 4, 6);

    if (sendto(fd, recv_buf, 4 * sizeof(char), 0, &saddr, sizeof(saddr)) < 0)
    {
      die(EXIT_FAILURE, "failed to send packet");
    }
    fprintf(stderr, "sent one packet for %s", argv[k]);
  }

  struct sockaddr_in sender = {0};
  socklen_t sender_size = sizeof(sender);
  struct pollfd fds[] = {
      {fd, POLLIN | POLLHUP, 0},
  };
  while (poll(fds, 1, -1) > 0)
  {
    if ((fds[0].revents & POLLHUP) || (fds[0].revents & POLLERR))
    {
      die(POLL_ERR, "failed while polling");
    }
    if (fds[0].revents & POLLIN)
    {
      int rc = 0;
      while ((rc = recvfrom(fd, &recv_buf, sizeof(recv_buf), 0, &sender, &sender_size)) > 0)
      {
        // recv file chunk
        fprintf(stderr, "received %d bytes\n", rc);
      }
    }
  }

  // reassemble chunks

  return 0;
}

void die(int code, const char *msg)
{
  write(STDERR_FILENO, msg, strlen(msg));
  if (fd > 0)
    close(fd);
  exit(code);
}
