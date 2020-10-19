package com.jaxrtech.bolt.messages;

import com.fasterxml.jackson.databind.annotation.JsonSerialize;
import com.jaxrtech.bolt.Message;
import com.jaxrtech.bolt.MessageKind;

@JsonSerialize
public class FileListingRequest implements Message {
    public static final MessageKind KIND = MessageKind.REQUEST_LISTING;

    @Override
    public String getKind() {
        return KIND.getCode();
    }
}

