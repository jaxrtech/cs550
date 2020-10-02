#include "bolt/binfmt.h"
#include <bolt/panic.h>
#include <cstring>

extern "C" {

uint16_t
BF_recomputePhysicalSize_single(BF_MessageElement *self)
{
    uint16_t size = 0;
    switch (self->type) {
        case BF_UINT8:
            size += sizeof(uint8_t);
            self->cached_size = size;
            return size;

        case BF_UINT16:
            size += sizeof(uint16_t);
            self->cached_size = size;
            return size;

        case BF_INT32:
            size += sizeof(int);
            self->cached_size = size;
            return size;

        case BF_LSTRING:
            // make sure to include the null terminator
            self->lstring.cached_strlen = strlen(self->lstring.str) + 1;
            size += sizeof(self->lstring.cached_strlen);
            size += self->lstring.cached_strlen;
            self->cached_size = size;
            return size;

        case BF_ARRAY_UINT8:
            size += sizeof(self->array_u8.len);
            size += self->array_u8.len * sizeof(uint8_t);
            self->cached_size = size;
            return size;

        case BF_ARRAY_MSG: {
            const uint8_t n = self->array_msg.data_count;
            size += sizeof(uint8_t);// store number of data elements
            for (int i = 0; i < n; i++) {
                size += BF_recomputePhysicalSize_single(&self->array_msg.data[i]);
            }
            self->cached_size = size;
            return size;
        }
    }

    PANIC("unhandled BF_MessageElement type");
    return -1;
}

uint64_t
BF_recomputePhysicalSize(BF_MessageElement *arr, uint8_t num_elements)
{
    uint64_t size = 0;
    uint32_t i;
    for (i = 0; i < num_elements; i++) {
        size += BF_recomputePhysicalSize_single(&arr[i]);
    }
    return size;
}

uint64_t
BF_write_single(BF_MessageElement *self, void *buffer)
{
    BF_recomputePhysicalSize_single(self);
    void *buffer_orig = buffer;

    switch (self->type) {
        case BF_UINT8:
            RM_BUF_WRITE(buffer, uint8_t, self->u8);
            break;

        case BF_UINT16:
            RM_BUF_WRITE(buffer, uint16_t, self->u16);
            break;

        case BF_INT32:
            RM_BUF_WRITE(buffer, int32_t, self->i32);
            break;

        case BF_LSTRING:
            self->lstring.cached_strlen = strlen(self->lstring.str) + 1;
            RM_BUF_WRITE_LSTRING(buffer, self->lstring.str, self->lstring.cached_strlen);
            break;

        case BF_ARRAY_UINT8:
            RM_BUF_WRITE_LSTRING(buffer, self->array_u8.buf, self->array_u8.len);
            break;

        case BF_ARRAY_MSG: {
            const uint8_t n = self->array_msg.data_count;
            RM_BUF_WRITE(buffer, uint8_t, n);
            for (int i = 0; i < n; i++) {
                buffer = (char *) buffer + BF_write_single(&self->array_msg.data[i], buffer);
            }
            break;
        }

        default:
            PANIC("unsupported binfmt data type: %d", self->type);
    }

    return ((char *) buffer) - ((char *) buffer_orig);
}

uint64_t
BF_write(BF_MessageElement *arr, void *buffer, uint32_t num_elements)
{
    void *buffer_orig = buffer;

    for (uint32_t i = 0; i < num_elements; i++) {
        buffer = (char *) buffer + BF_write_single(&arr[i], buffer);
    }

    return ((char *) buffer) - ((char *) buffer_orig);
}

uint16_t
BF_read_single(BF_MessageElement *self, void *buffer)
{
    void *buffer_orig = buffer;

    switch (self->type) {
        case BF_UINT8:
            RM_BUF_READ(buffer, uint8_t, self->u8);
            break;

        case BF_UINT16:
            RM_BUF_READ(buffer, uint16_t, self->u16);
            break;

        case BF_INT32:
            RM_BUF_READ(buffer, int32_t, self->i32);
            break;

        case BF_LSTRING:
            RM_BUF_READ(buffer, uint8_t, self->lstring.cached_strlen);
            self->lstring.str = (char *) buffer;
            buffer = (char *) buffer + self->lstring.cached_strlen;
            break;

        case BF_ARRAY_UINT8:
            RM_BUF_READ(buffer, uint8_t, self->array_u8.len);
            self->array_u8.buf = (uint8_t *) buffer;
            buffer = (char *) buffer + self->array_u8.len;
            break;

        case BF_ARRAY_MSG: {
            RM_BUF_READ(buffer, uint8_t, self->array_msg.data_count);
            const uint8_t n = self->array_msg.data_count;
            const uint8_t k = self->array_msg.type_count;
            self->array_msg.data = (BF_MessageElement *) malloc(sizeof(BF_MessageElement) * n);
            for (int i = 0; i < n; i++) {
                const uint8_t ti = i % k;
                memcpy(&self->array_msg.data[i], &self->array_msg.type[ti], sizeof(BF_MessageElement));
                buffer = (char *) buffer + BF_read_single(&self->array_msg.data[i], buffer);
            }
            break;
        }

        default:
            PANIC("unsupported binfmt data type: %d", self->type);
    }

    return ((char *) buffer) - ((char *) buffer_orig);
}

uint16_t
BF_read(BF_MessageElement *arr, void *buffer, uint8_t num_elements)
{
    void *buffer_orig = buffer;

    for (int i = 0; i < num_elements; i++) {
        buffer = (char *) buffer + BF_read_single(&arr[i], buffer);
    }

    return ((char *) buffer) - ((char *) buffer_orig);
}

}