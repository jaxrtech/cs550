#include "bolt/messages.h"

#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wwritable-strings"
extern "C" {

const BOLT_REQUEST_ACTION_T BOLT_REQUEST_ACTION_FORMAT = {
        .requestAction = {
                .name = "bolt_req_action",
                .type = BF_LSTRING,
        },
        .requestFileMd5 = {
                .name = "bolt_req_file_md5",
                .type = BF_ARRAY_UINT8,
        }
};

const BOLT_FILE_INFO_FORMAT_T BOLT_FILE_INFO_FORMAT = {
        .fileName = {
                .name = "bolt_file_name",
                .type = BF_LSTRING,
        },
        .fileSize = {
                .name = "bolt_file_size",
                .type = BF_INT32,
        },
        .fileMd5 = {
                .name = "bolt_file_md5",
                .type = BF_ARRAY_UINT8,
        },
};

const BOLT_FILE_LISTING_FORMAT_T BOLT_FILE_LISTING_FORMAT = {
        .fileEntries = {
                .name = "bolt_file_entries",
                .type = BF_ARRAY_MSG,
        },
};

const BOLT_FILE_DATA_FORMAT_T BOLT_FILE_DATA_FORMAT = {
        .fileMd5 = {
                .name = "bolt_file_md5",
                .type = BF_ARRAY_UINT8,
        },
        .fileOffset = {
                .name = "bolt_file_offset",
                .type = BF_INT32,
        },
        .fileData = {
                .name = "bolt_file_data",
                .type = BF_ARRAY_UINT8,
        },
};

}
#pragma clang diagnostic pop