package com.jaxrtech.bolt;

import com.jaxrtech.bolt.messages.FileChunkResponse;
import com.jaxrtech.bolt.messages.FileFetchRequest;
import com.jaxrtech.bolt.messages.FileListingRequest;
import com.jaxrtech.bolt.messages.FileListingResponse;

import java.util.HashMap;
import java.util.Map;
import java.util.Optional;

public class MessageRegistration {
    private final Map<String, Class<? extends Message>> formats = new HashMap<>();

    public static MessageRegistration defaultSet() {
        return new MessageRegistration()
                .register(FileListingRequest.KIND, FileListingRequest.class)
                .register(FileListingResponse.KIND, FileListingResponse.class)
                .register(FileFetchRequest.KIND, FileFetchRequest.class)
                .register(FileChunkResponse.KIND, FileChunkResponse.class);
    }

    public <T extends Message> MessageRegistration register(MessageKind kind, Class<T> clazz) {
        if (formats.containsKey(kind.getCode())) {
            throw new IllegalArgumentException("Cannot register message with duplicate 'kind' of '" + kind + "'.");
        }

        formats.put(kind.getCode(), clazz);
        return this;
    }

    public Optional<Class<? extends Message>> get(String code) {
        return Optional.ofNullable(formats.getOrDefault(code, null));
    }
}
