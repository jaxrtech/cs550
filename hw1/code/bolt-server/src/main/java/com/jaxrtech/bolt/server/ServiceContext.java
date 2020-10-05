package com.jaxrtech.bolt.server;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.jaxrtech.bolt.BufferContext;
import com.jaxrtech.bolt.MessageWriter;

import java.net.SocketAddress;
import java.util.Map;
import java.util.concurrent.ExecutorService;

class ServiceContext {
    private final Map<SocketAddress, BufferContext> clients;
    private final ObjectMapper mapper;
    private final MessageWriter writer;

    public ServiceContext(
            Map<SocketAddress, BufferContext> clients,
            ObjectMapper mapper, MessageWriter writer) {
        this.clients = clients;
        this.mapper = mapper;
        this.writer = writer;
    }

    public Map<SocketAddress, BufferContext> getClients() {
        return clients;
    }

    public ObjectMapper getMapper() {
        return mapper;
    }

    public MessageWriter getMessageWriter() {
        return writer;
    }
}
