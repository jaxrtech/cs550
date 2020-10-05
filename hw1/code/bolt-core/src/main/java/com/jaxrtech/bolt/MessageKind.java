package com.jaxrtech.bolt;

public enum MessageKind {
    REQUEST_LISTING("LIST"),
    RESPOND_LISTING("FILES");

    private final String code;

    MessageKind(String code) {
        this.code = code;
    }

    public String getCode() {
        return code;
    }
}
