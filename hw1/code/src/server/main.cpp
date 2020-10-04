#include <cstdlib>
#include <cstdio>
#include <vector>
#include <atomic>

#include <unistd.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <sys/epoll.h>

#include <algorithm>
#include <bolt/panic.h>
#include <err.h>
#include <map>
#include <sstream>
#include <string>
#include <sys/stat.h>

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
#define BOLT_DEFAULT_PORT 9000

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


struct buf_read_state {
    buf_read_state()
            : buffer(4096)
            , next_pos(0)
            , is_done(false)
    {}

    void *offset_ptr() { return buffer.data() + next_pos; }
    [[nodiscard]] uint64_t buffer_free_capacity() const { return buffer.capacity() - next_pos; }
    void advance(uint64_t step) { next_pos += step; }

    std::vector<std::byte> buffer;
    uint64_t next_pos;
    bool is_done;
};

int main(int argc, char **argv)
{
    char *executable_name = argv[0];
    PANIC_IF_NULL(executable_name);

    argc--;
    argv++;
    std::vector<std::string> args;
    for (int i = 0; i < argc; i++) {
        args.emplace_back(argv[i]);
    }

    std::array<std::string,3> help_options = {"-h", "--help"};
    auto is_help = [&](const std::string &a) {
        return std::any_of(
                help_options.begin(),
                help_options.end(),
                [&](const std::string &b) { return a == b; });
    };

    if (argc > 1 || std::any_of(args.begin(), args.end(), is_help)) {
        fprintf(stderr, "\nUsage: %s [FILE_DIRECTORY]\n\n", executable_name);
        fprintf(stderr, "A parallel file transfer server.\n\n");
        fprintf(stderr, "Developed by Josh Bowden (A20374650)\n");
        fprintf(stderr, "Illinois Institute of Technology - CS550 - Fall 2020\n");
        return 1;
    }

    std::string serve_path;
    if (argc > 0) {
        char *serve_path_raw = argv[0];
        struct stat sb = {};
        bool serve_path_exists = stat(serve_path_raw, &sb) == 0;
        bool serve_path_is_dir = S_ISDIR(sb.st_mode);

        if (!serve_path_exists) {
            fprintf(stderr, "error: directory does not exist: '%s'\n", serve_path_raw);
            return 1;
        }

        if (!serve_path_is_dir) {
            fprintf(stderr, "error: expected path to be a directory but got a file: '%s'\n", serve_path_raw);
            return 1;
        }
    }
    else {
        serve_path = ".";
        fprintf(stderr, "warn: serving from current directory since no path specified\n");
    }

    //

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
    addr.sin_port = htons(BOLT_DEFAULT_PORT);

    result = bind(server_fd, reinterpret_cast<const sockaddr *>(&addr), sizeof(addr));
    PANIC_IF_NEG_WITH_ERRNO(result, "bind");

    // Mark as nonblocking fd
    bolt::internal::set_nonblocking(server_fd);

    // Listen on socket
    result = listen(server_fd, SOMAXCONN);
    PANIC_IF_NEG_WITH_ERRNO(result, "listen");
    printf("info: listening on localhost:%d\n", BOLT_DEFAULT_PORT);

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
    std::map<int32_t, buf_read_state> client_states;
    std::vector<struct epoll_event> events(MAXEVENTS);
#define BUFSIZE (4096)
    uint8_t buf[BUFSIZE] = {};

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
            auto &read_state = client_states[event_fd];
            while (true) {
                printf("debug: [%d] read\n", event_fd);
                ssize_t num_bytes = read(event_fd, read_state.offset_ptr(), read_state.buffer_free_capacity());
                printf("debug: [%d] got %ld bytes\n", event_fd, num_bytes);

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

                read_state.advance(num_bytes);
            }
        }
    }
#pragma clang diagnostic pop

    return 0;
}
