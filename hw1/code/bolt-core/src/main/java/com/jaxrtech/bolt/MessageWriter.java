package com.jaxrtech.bolt;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.jaxrtech.bolt.messages.CustomMessagePacking;
import org.msgpack.core.MessagePack;

import java.io.IOException;
import java.nio.ByteBuffer;

public class MessageWriter {
    private final ObjectMapper objectMapper;

    public MessageWriter(ObjectMapper objectMapper) {
        this.objectMapper = objectMapper;
    }

    public void writeTo(Message message, ByteBuffer buffer) throws IOException {
        byte[] encoded;

        if (message instanceof CustomMessagePacking) {
            var writer = (CustomMessagePacking) message;
            var packer = MessagePack.newDefaultBufferPacker();
            writer.pack(packer);
            encoded = packer.toByteArray();
            packer.close();
        }
        else {
            encoded = objectMapper.writeValueAsBytes(message);
        }

        buffer.putInt(encoded.length);
        buffer.put(encoded);
    }
}
