package com.jaxrtech.bolt.client;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.jaxrtech.bolt.BufferContext;
import com.jaxrtech.bolt.Message;
import com.jaxrtech.bolt.MessageContext;
import com.jaxrtech.bolt.MessageReader;
import com.jaxrtech.bolt.MessageRegistration;
import com.jaxrtech.bolt.MessageWriter;
import com.jaxrtech.bolt.messages.FileListingRequest;
import com.jaxrtech.bolt.messages.FileListingResponse;
import org.msgpack.jackson.dataformat.MessagePackFactory;

import java.io.IOException;
import java.net.InetSocketAddress;
import java.nio.ByteBuffer;
import java.nio.channels.SelectionKey;
import java.nio.channels.Selector;
import java.nio.channels.SocketChannel;
import java.util.Iterator;
import java.util.Set;

class Main {

    private static void handler(MessageContext envelope, Object ignored) {
        Message message = envelope.getMessage();
        var kind = message.getKind();

        if (kind.equals("FILES")) {
            FileListingResponse response = (FileListingResponse) message;
            response.getFiles().forEach(f -> {
                System.out.printf("> %s (%d bytes)\n", f.getName(), f.getSize());
            });
        }
    }

    public static void main(String[] args) throws IOException {
        ObjectMapper objectMapper = new ObjectMapper(new MessagePackFactory());
        MessageRegistration registration = MessageRegistration.defaultSet();
        MessageReader<Object> messageReader = new MessageReader<>(registration, objectMapper, null, Main::handler);
        BufferContext bufferContext = new BufferContext();
        MessageWriter writer = new MessageWriter(objectMapper);
        
        var selector = Selector.open();
        var socket = SocketChannel.open(new InetSocketAddress("localhost", 9000));
        socket.configureBlocking(true);
        socket.finishConnect();

        ByteBuffer buffer = bufferContext.getReadBuffer();
        writer.writeTo(new FileListingRequest(), buffer);
        buffer.flip();

        int numWrote = socket.write(buffer);
        System.out.println("Wrote " + numWrote + " bytes");
        buffer.clear();

        socket.configureBlocking(false);
        socket.register(selector, SelectionKey.OP_READ);

        while (true) {
            selector.select();
            Set<SelectionKey> selectedKeys = selector.selectedKeys();
            Iterator<SelectionKey> iter = selectedKeys.iterator();
            while (iter.hasNext()) {
                SelectionKey key = iter.next();
                iter.remove();

                if (key.isReadable()) {
                    messageReader.read(socket, bufferContext);
                }
            }
        }
    }
}