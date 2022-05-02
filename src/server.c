#include <stdlib.h>
#include <unistd.h>

#include <sys/socket.h>
#include <sys/types.h>
#include <netinet/in.h>

#include <string.h>
#include <errno.h>
#include <syslog.h>
#include <signal.h>

#include <poll.h>

void die();
void *generateResponse(const char *);
void handleSigint(int);

// Options.
char *logPath = NULL;
uint16_t port = 0;

int main(int argc, char * const *argv)
{
  setlogmask(LOG_UPTO(LOG_INFO));
  openlog(argv[0], LOG_CONS | LOG_PID | LOG_NDELAY | LOG_PERROR, LOG_USER);

  int c = 0;
  while ((c = getopt(argc, argv, "46dC:p:")) > 0)
  {
    switch (c)
    {
    case '4':
    case '6':
      break;
    case 'd':
      syslog(LOG_INFO, "daemonizing");
      break;
    case 'C':
      if (chdir(optarg) < 0)
        die();
      syslog(LOG_INFO, "set directory to %s", optarg);
      break;
    case 'p':
      if ((port = atoi(optarg)) < 0)
        die();
      break;
    default:
      syslog(LOG_INFO, "unknown argument passed");
      break;
    }
  }

  // Register signals.
  signal(SIGINT, handleSigint);

  // Initialize socket.
  int fd = socket(AF_INET, SOCK_DGRAM, 0);

  struct sockaddr_in addr;
  addr.sin_family = AF_INET;
  addr.sin_addr.s_addr = INADDR_ANY;
  addr.sin_port = htons(port);
  socklen_t addrlen = sizeof(addr);
  bind(fd, (struct sockaddr *)&addr, sizeof(addr));

  // Poll for input, send responses.
  char recvbuf[4096] = {0};
  ssize_t recvd = 0;
  struct pollfd fds[] = {
      {fd, POLLIN | POLLHUP | POLLERR, 0},
  };

  syslog(LOG_INFO, "Started logging at port %d", port);
  while (poll(fds, 1, -1) >= 0)
  {
    if (fds[0].revents & POLLIN)
    {
      // TODO: handle if the receive fails.
      if ((recvd = recvfrom(fd, recvbuf, sizeof(recvbuf), 0, (struct sockaddr *)&addr, &addrlen)) < 0)
        return 1;

      uint8_t meta = recvbuf[0];
      char *data = recvbuf + 1;
      syslog(LOG_INFO, "Packet meta is %d, data is %s", meta, data);

      syslog(LOG_INFO, "Reply to %d\n", addr.sin_port);
      memcpy(recvbuf, "STOP", 5);
      syslog(LOG_INFO, "Sent response %zd.\n", sendto(fd, recvbuf, 5, 0, (struct sockaddr *)&addr, addrlen));
    }
  }
  return 0;
}

void die()
{
  syslog(LOG_ERR, "%s", strerror(errno));
  exit(EXIT_FAILURE);
}

void handleSigint(int sigNum)
{
  switch (sigNum) {
  case SIGINT:
    break;
  }
  closelog();
  exit(EXIT_SUCCESS);
}
