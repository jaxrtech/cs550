package com.jaxrtech.bolt.client;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.msgpack.jackson.dataformat.MessagePackFactory;

import java.io.IOException;
import java.net.InetSocketAddress;
import java.nio.ByteBuffer;
import java.nio.channels.SelectionKey;
import java.nio.channels.Selector;
import java.nio.channels.ServerSocketChannel;
import java.nio.channels.SocketChannel;
import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.Iterator;
import java.util.Map;
import java.util.Set;

class Main {

    public static void main(String[] args) throws IOException {
        Selector selector = Selector.open();

        var serverSocket = SocketChannel.open(new InetSocketAddress("localhost", 9000));
        serverSocket.configureBlocking(false);
        serverSocket.finishConnect();

        ByteBuffer buffer = ByteBuffer.allocate(32 * 1024);

        ObjectMapper objectMapper = new ObjectMapper(new MessagePackFactory());

        Map<String, Object> map = new HashMap<>();
        map.put("kind", "LIST");
        byte[] message = objectMapper.writeValueAsBytes(map);
        buffer.put(message);
        buffer.flip();

        int numWrote = serverSocket.write(buffer);
        System.out.println("Wrote " + numWrote + " bytes");

        while (true) {
            selector.select();
            Set<SelectionKey> selectedKeys = selector.selectedKeys();
            Iterator<SelectionKey> iter = selectedKeys.iterator();
            while (iter.hasNext()) {
                SelectionKey key = iter.next();
                iter.remove();

                if (key.isReadable()) {
                    SocketChannel client = (SocketChannel) key.channel();
                    int read = client.read(buffer);
                    System.out.println("Read " + read + " bytes");
                }
            }
        }
    }
}