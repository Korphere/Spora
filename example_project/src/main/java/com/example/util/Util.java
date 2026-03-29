package com.example.util;

public class Util {
    public static JsonObject getNewJsonObject(){
        JsonObject json = new JsonObject();
        json.addProperty("foo", "baa");
        return json;
    }
}