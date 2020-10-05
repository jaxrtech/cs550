package com.jaxrtech.bolt;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;
import com.fasterxml.jackson.annotation.JsonProperty.Access;

@JsonIgnoreProperties(ignoreUnknown = true)
public interface Message {
    @JsonProperty(value = "kind", access = Access.READ_ONLY)
    String getKind();
}
