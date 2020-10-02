#pragma once
#include "binfmt.h"

extern "C" {

typedef struct PACKED_STRUCT BOLT_REQUEST_ACTION_T {
    BF_MessageElement requestAction;
    BF_MessageElement requestFileMd5;
} BOLT_REQUEST_ACTION_T;

typedef struct PACKED_STRUCT BOLT_FILE_INFO_FORMAT_T {
    BF_MessageElement fileName;
    BF_MessageElement fileSize;
    BF_MessageElement fileMd5;
} BOLT_FILE_INFO_FORMAT_T;

typedef struct PACKED_STRUCT BOLT_FILE_LIST_FORMAT_T {
    BF_MessageElement fileEntries;
} BOLT_FILE_LISTING_FORMAT_T;

typedef struct PACKED_STRUCT BOLT_FILE_DATA_FORMAT_T {
    BF_MessageElement fileMd5;
    BF_MessageElement fileOffset;
    BF_MessageElement fileData;
} BOLT_FILE_DATA_FORMAT_T;

extern const BOLT_REQUEST_ACTION_T BOLT_REQUEST_ACTION_FORMAT;
extern const BOLT_FILE_INFO_FORMAT_T BOLT_FILE_INFO_FORMAT;
extern const BOLT_FILE_LISTING_FORMAT_T BOLT_FILE_LISTING_FORMAT;
extern const BOLT_FILE_DATA_FORMAT_T BOLT_FILE_DATA_FORMAT;

}