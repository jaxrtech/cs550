#pragma once

#include <pthread.h>
#include <cstdlib>
#include <cstdio>
#include <cerrno>
#include <cstring>

#define WARN(msg, vargs...) \
    do { \
        fprintf(stderr, "warn: (tid %d) %s:%d in %s: " msg "\n", \
                (int32_t) pthread_self(), \
                __FILE__, \
                __LINE__, \
                __FUNCTION__, \
                ##vargs); \
        fflush(stderr); \
        exit(1); \
    } while (0)

#define PANIC(msg, vargs...) \
    do { \
        fprintf(stderr, "panic! (tid %d) %s:%d in %s: " msg "\n", \
                (int32_t) pthread_self(), \
                __FILE__, \
                __LINE__, \
                __FUNCTION__, \
                ##vargs); \
        fflush(stderr); \
        exit(1); \
    } while (0)

#define PANIC_IF_NULL(_VAR) \
    do { \
        if (_VAR == nullptr) { PANIC("`%s` cannot be null", "#_VAR#"); } \
    } while (0)

#define PANIC_IF_ZERO(_VAR) \
    do { \
        if (_VAR == 0) { PANIC("`%s` cannot be zero", "#_VAR#"); } \
    } while (0)

#define PANIC_IF_NEG_WITH_ERRNO(_VAR, _FUNCNAME) \
    do { \
        if (_VAR < 0) { \
            int err = errno; \
            char errmsg[256] = {}; \
            strerror_r(err, errmsg, 256); \
            PANIC("%s: %s", _FUNCNAME, errmsg); \
        } \
    } while (0)

#define WARN_IF_NEG_WITH_ERRNO(_VAR, _FUNCNAME) \
    do { \
        if (_VAR < 0) { \
            int err = errno; \
            char errmsg[256] = {}; \
            strerror_r(err, errmsg, 256); \
            WARN("%s: %s", _FUNCNAME, errmsg); \
        } \
    } while (0)


#define PANIC_WITH_ERRNO(_FUNCNAME) \
    do { \
        int err = errno; \
        char errmsg[256] = {}; \
        strerror_r(err, errmsg, sizeof(errmsg)); \
        PANIC("%s: %s", _FUNCNAME, errmsg); \
    } while (0)
