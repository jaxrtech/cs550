package com.jaxrtech.bolt.messages;

import com.fasterxml.jackson.annotation.JsonCreator;
import com.fasterxml.jackson.annotation.JsonProperty;
import com.fasterxml.jackson.databind.annotation.JsonSerialize;
import com.jaxrtech.bolt.Message;
import com.jaxrtech.bolt.MessageKind;
import org.msgpack.core.MessagePacker;

import java.io.IOException;
import java.nio.ByteBuffer;

public class FileChunkResponse implements CustomMessagePacking {
    public static final MessageKind KIND = MessageKind.STREAM_CHUNK;
    private final String path;
    private final long offset;
    private final long size;
    private final ByteBuffer buffer;

    public FileChunkResponse(
            String path,
            long offset,
            long size,
            ByteBuffer buffer) {
        this.path = path;
        this.offset = offset;
        this.size = size;
        this.buffer = buffer;
    }

    @JsonCreator(mode = JsonCreator.Mode.PROPERTIES)
    public FileChunkResponse(
            @JsonProperty("path") String path,
            @JsonProperty("offset") long offset,
            @JsonProperty("size") long size,
            @JsonProperty("buffer") byte[] buffer) {
        this.path = path;
        this.offset = offset;
        this.size = size;
        this.buffer = ByteBuffer.wrap(buffer);
    }

    @Override
    public String getKind() {
        return KIND.getCode();
    }

    public String getPath() {
        return path;
    }

    public long getOffset() {
        return offset;
    }

    public ByteBuffer getBuffer() {
        return buffer;
    }

    public long getSize() {
        return size;
    }

    @Override
    public void pack(MessagePacker packer) throws IOException {
        byte[] backingBuffer;
        try {
            backingBuffer = buffer.array();
        }
        catch (UnsupportedOperationException ex) {
            throw new IllegalStateException("Cannot use buffer out-of-heap buffer. Avoid using 'ByteBuffer.allocateDirect' with this function.", ex);
        }
        packer.packMapHeader(4)
                .packString("kind").packString(getKind())
                .packString("path").packString(path)
                .packString("offset").packLong(offset);

        packer.packString("buffer");
        if (buffer.position() > 0) {
            buffer.flip();
        }
        packer.packBinaryHeader(buffer.limit());
        packer.writePayload(backingBuffer, 0, buffer.limit());
    }
}
