cmake_minimum_required(VERSION 3.17)
project(bolt)

set(CMAKE_CXX_STANDARD 17)

# server
add_executable(bolt_server
        src/server/main.cpp)

target_include_directories(bolt_server PRIVATE include/)

# client
add_executable(bolt_client
        src/client/main.cpp)

target_include_directories(bolt_client PRIVATE include/)
