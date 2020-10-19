# BOLT

CS550
HW 1
Joshua Bowden (A20374650)

## Introduction

This project is a Java `epoll()` event-loop based network implementation of a file server.

It consists of a client and server implementation that use MessagePack as data format to send between the client and the server.

To make the client easy to use, the list of files is sent back and then displayed in a text interface.

For efficiency, only one socket is used per client between the server and the individual files are "chunked" to the same socket. The client is then in charge of making sure that each part of the file is written to the corresponding handler.


