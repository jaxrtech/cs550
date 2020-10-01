#include <cstdlib>
#include <cstdio>
#include <vector>
#include <atomic>

#include <unistd.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <sys/epoll.h>

#include <bolt/panic.h>

namespace bolt::internal {

void set_nonblocking(int fd)
{
    uint32_t flags = fcntl(fd, F_GETFL, 0);
    PANIC_IF_NEG_WITH_ERRNO(flags, "fcntl");

    int result = fcntl(fd, F_SETFL, flags | O_NONBLOCK);
    PANIC_IF_NEG_WITH_ERRNO(result, "fcntl");
}

}

#define MAXEVENTS 64
#define BOLD_DEFAULT_PORT 9000

void bolt_read_pending(int event_fd, void *buf, size_t buf_size)
{
    int32_t result;

    while (true) {
        ssize_t num_bytes = read(event_fd, buf, buf_size);
        if (num_bytes < 0) {
            if (errno == EAGAIN || errno == EWOULDBLOCK) {
                printf("debug: [%d] read all data from client\n", event_fd);
                break;
            }

            PANIC_WITH_ERRNO("read");
        }

        if (num_bytes == 0) {
            printf("info: [%d] client disconnected\n", event_fd);
            result = close(event_fd);
            WARN_IF_NEG_WITH_ERRNO(result, "failed to close disconnected client socket. close:");
            break;
        }

        // TODO: Handle buf input with application logic
        fwrite(buf, sizeof(char), num_bytes, stdout);
    }
}

/**
 * Accepts as many new, pending connections as possible
 * @param epoll_fd  The `epoll` file descriptor
 * @param server_fd  The server TCP socket file descriptor
 */
void bolt_accept_pending(int32_t epoll_fd, int32_t server_fd)
{
    int32_t result;
    while (true) {
        struct sockaddr_in client_addr = {};
        socklen_t client_addr_len = sizeof(client_addr);
        int32_t client_fd = accept4(server_fd, reinterpret_cast<sockaddr *>(&client_addr), &client_addr_len, SOCK_NONBLOCK);
        if (client_fd < 0) {
            if (errno == EAGAIN || errno == EWOULDBLOCK) {
                // All pending connection requests have been accepted
                break;
            }

            PANIC_WITH_ERRNO("accept");
        }

        printf("info: accepted new connection on fd %d\n", client_fd);
        struct epoll_event add_event = {};
        add_event.data.fd = client_fd;
        add_event.events = EPOLLIN | EPOLLET;
        result = epoll_ctl(epoll_fd, EPOLL_CTL_ADD, client_fd, &add_event);
        PANIC_IF_NEG_WITH_ERRNO(result, "epoll_ctl(EPOLL_CTL_ADD)");
    }
}

int main()
{
    int32_t result;

    // Create socket
    int32_t server_fd = socket(AF_INET, SOCK_STREAM, 0);
    PANIC_IF_NEG_WITH_ERRNO(server_fd, "socket");

    uint32_t enable_flag = 1;
    result = setsockopt(server_fd, SOL_SOCKET, SO_REUSEADDR, &enable_flag, sizeof(enable_flag));
    PANIC_IF_NEG_WITH_ERRNO(result, "setsockopt(SO_REUSEADDR)");

    result = setsockopt(server_fd, SOL_SOCKET, SO_REUSEPORT, &enable_flag, sizeof(enable_flag));
    PANIC_IF_NEG_WITH_ERRNO(result, "setsocketopt(SO_REUSEPORT)");

    // Bind socket
    struct sockaddr_in addr = {};
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    addr.sin_port = htons(BOLD_DEFAULT_PORT);

    result = bind(server_fd, reinterpret_cast<const sockaddr *>(&addr), sizeof(addr));
    PANIC_IF_NEG_WITH_ERRNO(result, "bind");

    // Mark as nonblocking fd
    bolt::internal::set_nonblocking(server_fd);

    // Listen on socket
    result = listen(server_fd, SOMAXCONN);
    PANIC_IF_NEG_WITH_ERRNO(result, "listen");
    printf("info: listening on localhost:%d\n", BOLD_DEFAULT_PORT);

    // Create epoll fd
    int32_t epollfd = epoll_create1(0);
    PANIC_IF_NEG_WITH_ERRNO(epollfd, "epoll_create1");

    // Mark server socket for reading and edge-triggering
    struct epoll_event tmp_epoll_event = {};
    tmp_epoll_event.data.fd = server_fd;
    tmp_epoll_event.events = EPOLLIN | EPOLLET;

    result = epoll_ctl(epollfd, EPOLL_CTL_ADD, server_fd, &tmp_epoll_event);
    PANIC_IF_NEG_WITH_ERRNO(result, "epoll_ctl(EPOLL_CTL_ADD)");

    // Event loop to receive events
    std::vector<struct epoll_event> events(MAXEVENTS);
    uint8_t buf[4096] = {};

#pragma clang diagnostic push
#pragma ide diagnostic ignored "EndlessLoop"
    while (true) {
        int32_t num_events = epoll_wait(epollfd, events.data(), MAXEVENTS, -1);
        PANIC_IF_NEG_WITH_ERRNO(num_events, "epoll_wait");

        for (int32_t i = 0; i < num_events; i++) {
            auto const& e = events[i];
            uint32_t flags = e.events;
            int event_fd = e.data.fd;

            if (flags & EPOLLERR || flags & EPOLLHUP || !(flags & EPOLLIN)) {
                fprintf(stderr, "error: bad epoll event descriptor flag on fd = %d\n", event_fd);
                result = close(event_fd);
                WARN_IF_NEG_WITH_ERRNO(result, "failed to close bad socket fd. close:");
                continue;
            }

            // Server socket event
            if (event_fd == server_fd) {
                bolt_accept_pending(epollfd, server_fd);
                continue;
            }

            // Client socket event, read from pending sockets
            bolt_read_pending(event_fd, buf, sizeof(buf));
        }
    }
#pragma clang diagnostic pop

    return 0;
}
