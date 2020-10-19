package com.jaxrtech.bolt;

public enum MessageKind {
    REQUEST_LISTING("LIST"),
    RESPOND_LISTING("FILES"),
    REQUEST_DOWNLOAD("FETCH"),
    STREAM_CHUNK("DATA");

    private final String code;

    MessageKind(String code) {
        this.code = code;
    }

    public String getCode() {
        return code;
    }

    @Override
    public String toString() {
        return getCode();
    }
}
