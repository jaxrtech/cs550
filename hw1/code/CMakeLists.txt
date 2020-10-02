cmake_minimum_required(VERSION 3.17)
project(bolt)

set(CMAKE_CXX_STANDARD 17)

set(BOLT_SOURCES_SHARED
        src/shared/binfmt.cpp
        src/shared/messages.cpp
        )

# server
add_executable(bolt_server
        ${BOLT_SOURCES_SHARED}
        src/server/main.cpp)

target_include_directories(bolt_server PRIVATE include/)

# client
add_executable(bolt_client
        ${BOLT_SOURCES_SHARED}
        src/client/main.cpp)

target_include_directories(bolt_client PRIVATE include/)
