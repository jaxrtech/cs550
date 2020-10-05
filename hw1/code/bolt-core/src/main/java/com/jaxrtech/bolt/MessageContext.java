package com.jaxrtech.bolt;

import java.nio.channels.SocketChannel;

public class MessageContext {
    private final Message message;
    private final SocketChannel channel;

    public MessageContext(Message message, SocketChannel channel) {
        this.message = message;
        this.channel = channel;
    }

    public Message getMessage() {
        return message;
    }

    public SocketChannel getChannel() {
        return channel;
    }
}
