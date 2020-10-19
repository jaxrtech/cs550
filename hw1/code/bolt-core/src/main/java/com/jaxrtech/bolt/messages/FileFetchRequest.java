package com.jaxrtech.bolt.messages;

import com.fasterxml.jackson.annotation.JsonCreator;
import com.fasterxml.jackson.annotation.JsonProperty;
import com.jaxrtech.bolt.Message;
import com.jaxrtech.bolt.MessageKind;

public class FileFetchRequest implements Message {
    public static final MessageKind KIND = MessageKind.REQUEST_DOWNLOAD;
    private final String path;

    @JsonCreator(mode = JsonCreator.Mode.PROPERTIES)
    public FileFetchRequest(
            @JsonProperty("path") String path) {
        this.path = path;
    }

    @Override
    public String getKind() {
        return KIND.getCode();
    }

    public String getPath() {
        return path;
    }
}
