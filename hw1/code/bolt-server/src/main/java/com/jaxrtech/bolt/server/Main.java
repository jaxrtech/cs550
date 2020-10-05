package com.jaxrtech.bolt.server;

import java.io.IOException;
import java.net.InetSocketAddress;
import java.net.SocketAddress;
import java.net.StandardSocketOptions;
import java.nio.ByteBuffer;
import java.nio.channels.*;
import java.util.Set;
import java.util.concurrent.*;
import java.util.function.BiConsumer;
import java.util.function.Consumer;

public class Main {

    public static final int DEFAULT_PORT = 9000;
    public static final int DEFAULT_BUFFER_SIZE = 32 * 1024;

    public static void main(String[] args) throws IOException {

        Selector selector = Selector.open();

        var server = ServerSocketChannel.open();
        var listenAddress = new InetSocketAddress("127.0.0.1", DEFAULT_PORT);
        server.bind(listenAddress);
        server.configureBlocking(false);
        server.register(selector, server.validOps(), null);

        System.out.println("Listening on " + listenAddress);

        while (true) {
            selector.select();
            Set<SelectionKey> readyKeys = selector.selectedKeys();

            var iter = readyKeys.iterator();
            while (iter.hasNext()) {
                var key = iter.next();

                try {
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

                        ByteBuffer buffer = ByteBuffer.allocate(DEFAULT_BUFFER_SIZE);
                        int read = client.read(buffer);
                        if (read < 0) {
                            client.close();
                            log(String.format("[%s] Client disconnected", remoteAddress));
                            continue;
                        }

                        log(String.format("[%s] Read %d bytes", remoteAddress, read));
                    }
                }
                finally {
                    iter.remove();
                }
            }
        }
    }

    private static void log(String str) {
        System.out.println(str);
    }
}
