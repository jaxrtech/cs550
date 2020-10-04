#pragma once

#include <cstdint>
#define PACKED_STRUCT __attribute__((__packed__))

extern "C" {

typedef enum BF_DataType {
    BF_UINT8,
    BF_UINT16,
    BF_INT32,
    BF_LSTRING,
    BF_ARRAY_UINT8,
    BF_ARRAY_MSG,
} BF_DataType;

struct BF_MessageElement;

typedef struct BF_MessageElement {
    char* name;
    BF_DataType type;
    uint16_t cached_size;
    union {
        uint8_t u8;
        uint16_t u16;
        int32_t i32;
        struct {
            uint8_t cached_strlen;
            char *str;
        } lstring;
        struct {
            uint32_t len;
            uint8_t *buf;
        } array_u8;
        struct {
            uint8_t type_count;
            const struct BF_MessageElement *type;
            uint8_t data_count;
            struct BF_MessageElement *data;
        } array_msg;
    };
} BF_MessageElement;

#define BF_LSTRING_MAX_STRLEN (UINT8_MAX)

#define BF_DEREF_U8  .u8
#define BF_DEREF_U16 .u16
#define BF_DEREF_I32 .i32
#define BF_DEREF_STR .lstring.str

#define BF_SET_U8(VAR) \
    do { \
        if ((VAR).type != BF_UINT8) { \
            PANIC("wrong message element type. expected BF_UINT8."); \
        } \
    } while (0); \
    (VAR) BF_DEREF_U8

#define BF_SET_U16(VAR) \
    do { \
        if ((VAR).type != BF_UINT16) { \
            PANIC("wrong message element type. expected BF_UINT16."); \
        } \
    } while (0); \
    (VAR) BF_DEREF_U16

#define BF_SET_I32(VAR) \
    do { \
        if ((VAR).type != BF_INT32) { \
            PANIC("wrong message element type. expected BF_INT32."); \
        } \
    } while (0); \
    (VAR) BF_DEREF_I32

#define BF_AS_U8(VAR) \
    (((VAR).type != BF_UINT8) \
        ? (uint8_t) PANIC("wrong message element type. expected BF_UINT8.") \
        : ((VAR) BF_DEREF_U8))

#define BF_AS_U16(VAR) \
    (((VAR).type != BF_UINT16) \
        ? (uint8_t) PANIC("wrong message element type. expected BF_UINT16.") \
        : ((VAR) BF_DEREF_U16))

#define BF_AS_I32(VAR) \
    (((VAR).type != BF_INT32) \
        ? (uint8_t) PANIC("wrong message element type. expected BF_INT32.") \
        : ((VAR) BF_DEREF_I32))

#define BF_SET_STR(VAR) \
    do { \
        if ((VAR).type != BF_LSTRING) { \
            PANIC("wrong message element type. expected BF_LSTRING."); \
        } \
    } while (0); \
    (VAR) BF_DEREF_STR

#define BF_AS_STR(VAR) \
    (((VAR).type != BF_LSTRING) \
        ? (char *) PANIC("wrong message element type. expected BF_LSTRING.") \
        : ((VAR) BF_DEREF_STR))

#define BF_STRLEN(VAR) \
    (((VAR).type != BF_LSTRING) \
        ? (int) PANIC("wrong message element type. expected BF_LSTRING.") \
        : ((VAR).lstring.cached_strlen))

#define BF_SET_ARRAY_U8(VAR, PTR, N) \
    do { \
        if ((VAR).type != BF_ARRAY_UINT8) { \
            PANIC("wrong message element type. expected BF_ARRAY_UINT8."); \
        } \
        (VAR).array_u8.buf = (PTR); \
        (VAR).array_u8.len = (N); \
    } while (0)

#define BF_AS_ARRAY_U8(VAR) \
    (((VAR).type != BF_ARRAY_UINT8) \
        ? (uint8_t *) PANIC("wrong message element type. expected BF_ARRAY_UINT8.") \
        : ((VAR).array_u8.buf))

#define BF_ARRAY_U8_LEN(VAR) \
    (((VAR).type != BF_ARRAY_UINT8) \
        ? (int) PANIC("wrong message element type. expected BF_ARRAY_UINT8.") \
        : ((VAR).array_u8.len))

#define BF_SET_ARRAY_MSG(VAR, PTR, LEN_BYTES) \
    do { \
        if ((VAR).type != BF_ARRAY_MSG) { \
            PANIC("wrong message element type. expected BF_ARRAY_LMSG."); \
        } \
        (VAR).array_msg.data = (BF_MessageElement *) (PTR); \
        (VAR).array_msg.data_count = (LEN_BYTES) / sizeof(BF_MessageElement); \
    } while (0)

#define BF_AS_ARRAY_MSG(VAR) \
    (((VAR).type != BF_ARRAY_MSG) \
        ? (BF_MessageElement *) PANIC("wrong message element type. expected BF_ARRAY_MSG.") \
        : ((VAR).array_msg.data))

#define RM_BUF_READ(BUFFER, TYPE, DEST) \
    do { \
        DEST = *(TYPE *)(BUFFER); \
        BUFFER = ((char *) (BUFFER)) + sizeof(TYPE); \
    } while (0)

#define RM_BUF_READ_OR_RETURN_ZERO(BUFFER, LEN, TYPE, DEST) \
    do { \
        if (sizeof(TYPE) < (LEN)) { \
            return 0; \
        } \
        DEST = *(TYPE *) (BUFFER); \
        BUFFER = ((char *) (BUFFER)) + sizeof(TYPE); \
    } while (0)

//write val to buffer by casting val to TYPE pointer and dereferencing it
#define RM_BUF_WRITE(BUFFER, TYPE, VAL) \
    do { \
        *((TYPE *) (BUFFER)) = (TYPE)(VAL); \
        BUFFER = ((char *) (BUFFER)) + sizeof(TYPE); \
    } while (0)

#define RM_BUF_WRITE_FROM(BUFFER, PTR, LEN) \
    do { \
        memcpy((BUFFER), (PTR), (LEN)); \
        BUFFER = ((char *) (BUFFER)) + (LEN); \
    } while (0)

//writes the length of the string  and then writes the string (up to 255 chars)
#define RM_BUF_WRITE_LSTRING(BUFFER, STR_PTR, LEN) \
    do { \
        RM_BUF_WRITE((BUFFER), uint8_t, (LEN)); \
        RM_BUF_WRITE_FROM((BUFFER), (STR_PTR), (LEN)); \
    } while (0)

#define BF_NUM_ELEMENTS(BYTES) ((BYTES) / sizeof(BF_MessageElement))

uint16_t
BF_recomputePhysicalSize_single(BF_MessageElement *self);

uint64_t
BF_recomputePhysicalSize(BF_MessageElement *arr, uint8_t num_elements);

uint64_t
BF_write_single(BF_MessageElement *self, void *buffer);

uint64_t
BF_write(BF_MessageElement *arr, uint32_t num_elements, void *buffer);

uint64_t
BF_read_single(BF_MessageElement *self, void *buffer, uint64_t len);

uint64_t
BF_read(BF_MessageElement *arr, uint32_t num_elements, void *buffer, uint64_t len);

}