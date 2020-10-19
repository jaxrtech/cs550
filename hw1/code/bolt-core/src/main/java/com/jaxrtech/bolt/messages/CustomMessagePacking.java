package com.jaxrtech.bolt.messages;

import com.jaxrtech.bolt.Message;
import org.msgpack.core.MessagePacker;

import java.io.IOException;

public interface CustomMessagePacking extends Message {
    void pack(MessagePacker packer) throws IOException;
}
