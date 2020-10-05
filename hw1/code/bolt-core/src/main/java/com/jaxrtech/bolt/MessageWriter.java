package com.jaxrtech.bolt;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.jaxrtech.bolt.messages.FileListingRequest;

import java.io.IOException;
import java.nio.ByteBuffer;

public class MessageWriter {
    private final ObjectMapper objectMapper;

    public MessageWriter(ObjectMapper objectMapper) {
        this.objectMapper = objectMapper;
    }

    public void writeTo(Message message, ByteBuffer buffer) throws IOException {
        byte[] encoded = objectMapper.writeValueAsBytes(message);
        buffer.putInt(encoded.length);
        buffer.put(encoded);
    }

    public ByteBuffer write(Message message) throws IOException {
        byte[] encoded = objectMapper.writeValueAsBytes(message);
        ByteBuffer buffer = ByteBuffer.allocateDirect(encoded.length + 4);
        writeTo(message, buffer);
        return buffer;
    }
}
