package com.jaxrtech.bolt.server;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.jaxrtech.bolt.*;
import org.jetbrains.annotations.NotNull;

import java.io.IOException;
import java.net.InetSocketAddress;
import java.net.SocketAddress;
import java.nio.channels.SelectionKey;
import java.nio.channels.Selector;
import java.nio.channels.ServerSocketChannel;
import java.nio.channels.SocketChannel;
import java.util.*;

class Server {
    private final Map<SocketAddress, BufferContext> clientContexts;
    private final ServiceContext serviceContext;
    private final MessageReader messageReader;

    private Selector selector;
    private ServerSocketChannel server;

    public Server(
            MessageRegistration registration,
            ObjectMapper objectMapper) {
        this.clientContexts = new HashMap<>();
        this.serviceContext = new ServiceContext(clientContexts, objectMapper, new MessageWriter(objectMapper));
        this.messageReader = new MessageReader(registration, objectMapper);
    }

    public void listen(InetSocketAddress listenAddress) throws IOException {
        selector = Selector.open();

        server = ServerSocketChannel.open();
        server.bind(listenAddress);
        server.configureBlocking(false);
        server.register(selector, server.validOps(), null);
    }

    @NotNull
    public MessageContext readUntilNext() throws IOException {
        Optional<MessageContext> result;
        do {
            result = processNext();
        } while (result.isEmpty());

        return result.get();
    }

    private Optional<MessageContext> processNext() throws IOException {
        selector.select();
        Set<SelectionKey> readyKeys = selector.selectedKeys();

        var iter = readyKeys.iterator();
        while (iter.hasNext()) {
            var key = iter.next();
            iter.remove();

            if (key.isAcceptable()) {
                SocketChannel client = server.accept();
                if (client == null) {
                    continue;
                }
                client.configureBlocking(false);
                client.register(selector, SelectionKey.OP_READ);
                log("Accepted: " + client.getLocalAddress() + "\n");

            } else if (key.isReadable()) {
                SocketChannel client = (SocketChannel) key.channel();
                SocketAddress remoteAddress = client.getRemoteAddress();
                var ctx = clientContexts.computeIfAbsent(remoteAddress, x -> new BufferContext());
                return messageReader.read(client, ctx);
            }
        }

        return Optional.empty();
    }


    private static void log(String str) {
        System.out.println(str);
    }

    public ServiceContext getServiceContext() {
        return serviceContext;
    }
}
