package com.jaxrtech.bolt.server;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.jaxrtech.bolt.*;
import org.msgpack.jackson.dataformat.MessagePackFactory;

import java.io.IOException;
import java.net.InetSocketAddress;
import java.net.SocketAddress;
import java.nio.channels.SelectionKey;
import java.nio.channels.Selector;
import java.nio.channels.ServerSocketChannel;
import java.nio.channels.SocketChannel;
import java.util.HashMap;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ExecutorService;
import java.util.function.BiConsumer;

class Server {
    private final ExecutorService executor;
    private final MessageRegistration registration;
    private final Map<SocketAddress, BufferContext> clientContexts;
    private final ObjectMapper objectMapper;
    private final ServiceContext handlerContext;
    private final BiConsumer<MessageContext, ServiceContext> handler;
    private final MessageReader<ServiceContext> messageReader;

    private Selector selector;
    private ServerSocketChannel server;

    public Server(
            ExecutorService executor,
            MessageRegistration registration,
            BiConsumer<MessageContext, ServiceContext> handler) {
        this.objectMapper = new ObjectMapper(new MessagePackFactory());
        this.clientContexts = new HashMap<>();

        this.executor = executor;
        this.registration = registration;
        this.handler = handler;
        this.handlerContext = new ServiceContext(executor, clientContexts, objectMapper, new MessageWriter(objectMapper));
        this.messageReader = new MessageReader<>(registration, objectMapper, handlerContext, handler);
    }

    public void listen(InetSocketAddress listenAddress) throws IOException {
        selector = Selector.open();

        server = ServerSocketChannel.open();
        server.bind(listenAddress);
        server.configureBlocking(false);
        server.register(selector, server.validOps(), null);
    }

    public void run() throws IOException {
        while (true) {
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
                    messageReader.read(client, ctx);
                }
            }
        }
    }


    private static void log(String str) {
        System.out.println(str);
    }
}
