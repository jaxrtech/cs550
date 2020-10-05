package com.jaxrtech.bolt.messages;

import com.fasterxml.jackson.annotation.JsonCreator;
import com.fasterxml.jackson.annotation.JsonProperty;

public class FileInfo {
    private final String name;
    private final long size;

    @JsonCreator(mode = JsonCreator.Mode.PROPERTIES)
    public FileInfo(
            @JsonProperty("name") String name,
            @JsonProperty("size") long size) {
        this.name = name;
        this.size = size;
    }

    public String getName() {
        return name;
    }

    public long getSize() {
        return size;
    }
}
