package com.jaxrtech.bolt;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;

import java.io.IOException;
import java.net.SocketAddress;
import java.net.SocketException;
import java.nio.ByteBuffer;
import java.nio.channels.SocketChannel;
import java.util.Map;
import java.util.Optional;

public class MessageReader {
    private static final int PREFIX_SIZE = 4;
    private final MessageRegistration registration;
    private final ObjectMapper objectMapper;

    public MessageReader(
            MessageRegistration registration,
            ObjectMapper objectMapper) {
        this.registration = registration;
        this.objectMapper = objectMapper;
    }

    private static void log(String str, Object... args) {
//        System.err.printf(str, args);
    }

    public Optional<MessageContext> read(SocketChannel client, BufferContext ctx) throws IOException {
        SocketAddress remoteAddress = client.getRemoteAddress();
        var oldState = ctx.getReadState();

        ByteBuffer buffer = ctx.getReadBuffer();
        int read;
        try {
            read = client.read(buffer);
        } catch (SocketException ex) {
            try {
                client.close();
            } catch (IOException ignored) { }
            log(String.format("[%s] Client disconnected", remoteAddress));
            ex.printStackTrace();
            return Optional.empty();
        }
        if (read < 0) {
            client.close();
            log(String.format("[%s] Client disconnected", remoteAddress));
            return Optional.empty();
        }

        log(String.format("[%s] Read %d bytes", remoteAddress, read));

        if (oldState == BufferState.EMPTY) {
            // TODO: We're assuming at least 4 bytes are ready
            int len = buffer.getInt(0);
            ctx.setReadTarget(len);

            if ((buffer.position() - PREFIX_SIZE) < len) {
                // Not enough data to decode, will need to loop again
                return Optional.empty();
            }
        }

        // BufferState.AGAIN will simply loop again

        // Get the new read state since we got more data now
        if (ctx.getReadState() == BufferState.DONE) {
            try {
                Map<String, Object> deserialized = readMessage(buffer);

                Object raw = deserialized.getOrDefault("kind", null);
                if (!(raw instanceof String)) {
                    log("error: bad message format, expected 'key' to be 'String'");
                    return Optional.empty();
                }

                String kind = (String) raw;
                log("debug: got '%s' message", kind);

                Optional<Class<? extends Message>> decoder = registration.get(kind);
                if (decoder.isEmpty()) {
                    log("error: unhandled message of kind '%s'%n", kind);
                    return Optional.empty();
                }

                Message message = objectMapper.convertValue(deserialized, decoder.get());
                return Optional.of(new MessageContext(message, client));
            } finally {
                buffer.clear();
            }
        }

        return Optional.empty();
    }

    private Map<String, Object> readMessage(ByteBuffer buffer) throws IOException {
        return objectMapper.readValue(buffer.array(), PREFIX_SIZE, buffer.position(), new TypeReference<>() {
        });
    }
}
