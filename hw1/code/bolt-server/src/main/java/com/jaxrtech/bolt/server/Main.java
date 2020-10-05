package com.jaxrtech.bolt.server;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.jaxrtech.bolt.MessageContext;
import com.jaxrtech.bolt.MessageRegistration;
import com.jaxrtech.bolt.messages.FileInfo;
import com.jaxrtech.bolt.messages.FileListingResponse;
import org.msgpack.jackson.dataformat.MessagePackFactory;

import java.io.IOException;
import java.net.InetSocketAddress;
import java.nio.ByteBuffer;
import java.nio.channels.SocketChannel;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.*;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.stream.Collectors;

public class Main {

    public static final int DEFAULT_PORT = 9000;
    private static Path servePath;

    public static void main(String[] args) throws IOException {
        servePath = Paths.get("").toAbsolutePath();

        ExecutorService executor = Executors.newFixedThreadPool(Runtime.getRuntime().availableProcessors());
        var registration = MessageRegistration.defaultSet();
        var server = new Server(registration, new ObjectMapper(new MessagePackFactory()));
        var listenAddress = new InetSocketAddress("127.0.0.1", DEFAULT_PORT);

        server.listen(listenAddress);
        System.out.println("Listening on " + listenAddress);

        while (true) {
            var envelope = server.readUntilNext();
            var kind = envelope.getMessage().getKind();
            if (kind.equals("LIST")) {
                executor.submit(() -> handleList(envelope, server.getServiceContext()));
            }
        }
    }

    private static void handleList(MessageContext envelope, ServiceContext context) {
        try {
            SocketChannel channel = envelope.getChannel();
            List<FileInfo> files = Files.walk(servePath)
                    .filter(Files::isRegularFile)
                    .map(x -> {
                        long size = -1;
                        try {
                            size = Files.size(x);
                        } catch (IOException ignored) {
                        }

                        return new FileInfo(servePath.relativize(x).toString(), size);
                    })
                    .collect(Collectors.toList());

            var response = new FileListingResponse(files);
            ByteBuffer buffer = context.getMessageWriter().write(response);
            buffer.flip();
            channel.write(buffer);

        } catch (IOException e) {
            e.printStackTrace();
        }
    }

}
