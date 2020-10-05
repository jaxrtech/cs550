package com.jaxrtech.bolt;

import java.nio.ByteBuffer;

public class BufferContext {
    public static final int DEFAULT_BUFFER_SIZE = 32 * 1024;
    private final ByteBuffer readBuffer = ByteBuffer.allocate(DEFAULT_BUFFER_SIZE);
    private int readTarget = -1;

    public BufferContext() {
    }

    public void setReadTarget(int readTarget) {
        this.readTarget = readTarget;
    }

    public ByteBuffer getReadBuffer() {
        return readBuffer;
    }

    public BufferState getReadState() {
        int position = readBuffer.position();
        if (position <= 0) {
            return BufferState.EMPTY;
        }

        if (position < readTarget) {
            return BufferState.AGAIN;
        }

        return BufferState.DONE;
    }
}
