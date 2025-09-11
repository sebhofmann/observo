package de.paschty.obsero.monitor.zabbix;

import de.paschty.obsero.monitor.Classification;
import de.paschty.obsero.monitor.Message;
import java.time.Instant;
import java.util.Map;

public class ZabbixMessage implements Message {
    private final String title;
    private final String message;
    private final String hostname;
    private final Classification classification;
    private final Instant timestamp;
    private final Map<String, String> customFields;

    public ZabbixMessage(String title, String message, String hostname, Classification classification, Instant timestamp, Map<String, String> customFields) {
        this.title = title;
        this.message = message;
        this.hostname = hostname;
        this.classification = classification;
        this.timestamp = timestamp;
        this.customFields = customFields;
    }

    @Override
    public String getTitle() { return title; }
    @Override
    public String getMessage() { return message; }
    @Override
    public String hostname() { return hostname; }
    @Override
    public Classification getClassification() { return classification; }
    @Override
    public Instant getTimestamp() { return timestamp; }
    @Override
    public Map<String, String> getCustomFields() { return customFields; }
}

