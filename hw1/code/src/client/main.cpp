#include <cstdlib>
#include <cstdio>
#include <vector>
#include <atomic>
#include <optional>

#include <unistd.h>
#include <fcntl.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <sys/epoll.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <netdb.h>

#include <bolt/messages.h>
#include <bolt/panic.h>
#include <err.h>
#include <string>

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
#define BOLT_DEFAULT_SERVER "127.0.0.1"

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

class parse_address_result
{
    using Result = struct addrinfo;

public:
    static parse_address_result ok(struct addrinfo* tmp) {
        Result result = {};
        memcpy(&result, tmp, sizeof(struct addrinfo));
        return parse_address_result(true, result, 0, "");
    }

    static parse_address_result fail(int32_t code, const std::string &reason) {
        return parse_address_result(false, {}, code, reason);
    }

    [[nodiscard]] const std::string& reason() const {
        return reason_;
    }

    [[nodiscard]] std::string internal_reason() const {
        if (success_) { return ""; }
        return gai_strerror(status_);
    }

    std::optional<Result> try_get() {
        if (!success_) {
            return std::nullopt;
        }
        return std::optional(result_);
    }

    bool is_ok() const {
        return success_;
    }

    bool is_error() const {
        return !success_;
    }

private:
    parse_address_result(bool success, Result result, int32_t code, const std::string &reason)
        : success_(success)
        , result_(result)
        , status_(code)
        , reason_(reason)
    { }

    bool success_;
    int32_t status_;
    const std::string &reason_;
    struct addrinfo result_;
};

parse_address_result try_resolve_address(const std::string& input)
{
    struct addrinfo *tmp = nullptr;
    struct addrinfo hints = {};
    hints.ai_family = AF_INET;
    hints.ai_socktype = SOCK_STREAM;
    hints.ai_flags = AI_ADDRCONFIG | AI_NUMERICSERV;

    int status = getaddrinfo(input.c_str(), nullptr, &hints, &tmp);
    if (status != 0) {
        return parse_address_result::fail(status, "Failed to parse host '" + input + "'");
    }

    if (tmp == nullptr) {
        return parse_address_result::fail(status, "Unable to resolve host for '" + input + "'");
    }

    auto result = parse_address_result::ok(tmp);
    freeaddrinfo(tmp);

    return result;
}

void write_until_pending(
        int32_t client_fd,
        std::vector<std::byte> &buffer,
        uint64_t &write_pos)
{
    if (buffer.empty()) { return; }

    uint64_t remaining_len = (buffer.size() - 1)  - write_pos;
    uint64_t num_written = 0;
    while (remaining_len > 0) {
        num_written = write(client_fd, buffer.data() + write_pos, remaining_len);
        if (num_written < 0) {
            if (errno == EAGAIN || errno == EWOULDBLOCK) {
                break;
            }

            PANIC_WITH_ERRNO("write");
        }

        write_pos += num_written;
        remaining_len -= num_written;
    }
}


int main()
{
    int32_t result;

    // Create socket
    int32_t client_fd = socket(AF_INET, SOCK_STREAM | SOCK_NONBLOCK, 0);
    PANIC_IF_NEG_WITH_ERRNO(client_fd, "socket");

    // Create epoll fd
    int32_t epollfd = epoll_create1(0);
    PANIC_IF_NEG_WITH_ERRNO(epollfd, "epoll_create1");

    // Mark server socket for reading and edge-triggering
    struct epoll_event tmp_epoll_event = {};
    tmp_epoll_event.data.fd = client_fd;
    tmp_epoll_event.events = EPOLLIN | EPOLLOUT | EPOLLET;

    result = epoll_ctl(epollfd, EPOLL_CTL_ADD, client_fd, &tmp_epoll_event);
    PANIC_IF_NEG_WITH_ERRNO(result, "epoll_ctl(EPOLL_CTL_ADD)");

    // Make server address descriptor
    struct sockaddr_in server_addr = {};
    server_addr.sin_family = AF_INET;
    server_addr.sin_port = htons(BOLT_DEFAULT_PORT);

    // Convert server address from string to struct
    result = inet_aton(BOLT_DEFAULT_SERVER, &server_addr.sin_addr);
    PANIC_IF_ZERO(result);

    // Connect to server
    result = connect(client_fd, reinterpret_cast<const sockaddr *>(&server_addr), sizeof(server_addr));
    if (result < 0 && errno != EINPROGRESS) {
        PANIC_WITH_ERRNO("connect");
    }

    // Event loop to receive events
    std::vector<struct epoll_event> events(MAXEVENTS);
    uint8_t buf[4096] = {};
    int32_t num_events = 0;
    int32_t i = 0;

    num_events = epoll_wait(epollfd, events.data(), MAXEVENTS, 5000);
    for(i = 0; i < num_events; i++) {
        auto const& e = events[i];
        if (e.events & EPOLLIN) {
            printf("info: socket %d connected to server\n", e.data.fd);
        }
    }

    uint64_t write_pos = 0;
    int write_pending_len = 0;
    ssize_t num_written;
    std::vector<std::byte> write_buf(4096);

    if (write_pending_len <= 0) {
        // TODO: Do the next task
        fprintf(stderr, "debug: request `LIST` command");

        struct BOLT_REQUEST_ACTION_T request = BOLT_REQUEST_ACTION_FORMAT;
        BF_SET_STR(request.requestAction) = "LIST";
        uint64_t request_size = BF_recomputePhysicalSize((BF_MessageElement *) &request, BF_NUM_ELEMENTS(sizeof(request)));
        write_buf.resize(request_size);
        write_pos = 0;

        uint64_t wrote_size = BF_write((BF_MessageElement *) &request, write_buf.data(), BF_NUM_ELEMENTS(sizeof(request)));
        if (wrote_size > request_size) {
            PANIC("buffer overflow");
        }
    }

    write_until_pending(client_fd, write_buf, write_pos);

#pragma clang diagnostic push
#pragma ide diagnostic ignored "EndlessLoop"
    while (true) {
        num_events = epoll_wait(epollfd, events.data(), MAXEVENTS, -1);
        PANIC_IF_NEG_WITH_ERRNO(num_events, "epoll_wait");

        for (i = 0; i < num_events; i++) {
            auto const& e = events[i];
            uint32_t flags = e.events;
            int event_fd = e.data.fd;

            if (flags & EPOLLERR || flags & EPOLLHUP || !(flags & EPOLLIN)) {
                fprintf(stderr, "error: bad epoll event descriptor flag on fd = %d\n", event_fd);
                result = close(event_fd);
                WARN_IF_NEG_WITH_ERRNO(result, "failed to close bad socket fd. close:");
                continue; // TODO: reconnection logic
            }

            // Client socket event, read from pending sockets
            while (true) {
                ssize_t num_bytes = read(event_fd, buf, sizeof(buf));
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

            write_until_pending(client_fd, write_buf, write_pos);
            fprintf(stderr, "debug: finished loop\n");
        }
    }
#pragma clang diagnostic pop

    return 0;
}
