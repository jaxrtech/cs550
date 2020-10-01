cmake_minimum_required(VERSION 3.17)
project(bolt)

set(CMAKE_CXX_STANDARD 17)

add_executable(bolt
        src/main.cpp)

target_include_directories(bolt PRIVATE include/)
