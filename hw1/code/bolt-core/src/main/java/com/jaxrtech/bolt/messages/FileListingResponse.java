package com.jaxrtech.bolt.messages;

import com.fasterxml.jackson.annotation.JsonCreator;
import com.fasterxml.jackson.annotation.JsonProperty;
import com.jaxrtech.bolt.Message;
import com.jaxrtech.bolt.MessageKind;

import java.util.List;

public class FileListingResponse implements Message {
    public static final MessageKind KIND = MessageKind.RESPOND_LISTING;

    private final List<FileInfo> files;

    @JsonCreator(mode = JsonCreator.Mode.PROPERTIES)
    public FileListingResponse(
            @JsonProperty("files") List<FileInfo> files) {
        this.files = files;
    }

    @Override
    public String getKind() {
        return KIND.getCode();
    }

    public List<FileInfo> getFiles() {
        return files;
    }
}
