package de.paschty.observo.monitor.zabbix;

import de.paschty.observo.monitor.Classification;
import de.paschty.observo.monitor.Message;
import java.time.Instant;
import java.util.Map;

public class ZabbixMessage implements Message {
    private final String id;
    private final String title;
    private final String message;
    private final String hostname;
    private final Classification classification;
    private final Instant timestamp;
    private final Map<String, String> customFields;

    public ZabbixMessage(String id, String title, String message, String hostname, Classification classification, Instant timestamp, Map<String, String> customFields) {
        this.id = id;
        this.title = title;
        this.message = message;
        this.hostname = hostname;
        this.classification = classification;
        this.timestamp = timestamp;
        this.customFields = customFields;
    }

    @Override
    public String getId() { return id; }
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

    public static Classification mapClassification(org.json.JSONObject problem) {
        int severity = problem.optInt("severity", 0);
        int suppressed = problem.optInt("suppressed", 0);
        int acknowledged = problem.optInt("acknowledged", 0);
        int rns = problem.optInt("r_ns", 0);
        // Recovery-Event
        if (rns > 0) return Classification.RECOVERY;
        // acknowledged als ACKNOWLEDGED
        if (acknowledged == 1) return Classification.ACKNOWLEDGED;
        if (suppressed == 1) return Classification.INFO;
        return switch (severity) {
            case 0 -> Classification.UNKNOWN;
            case 1 -> Classification.INFO;
            case 2 -> Classification.WARNING;
            case 3 -> Classification.WARNING;
            case 4 -> Classification.CRITICAL;
            case 5 -> Classification.CRITICAL;
            default -> Classification.UNKNOWN;
        };
    }

    public static ZabbixMessage fromJson(org.json.JSONObject problem) {
        String id = problem.optString("eventid", "");
        String title = problem.optString("name", "Problem");
        String msg = problem.optString("eventid", "") + ": " + problem.optString("name", "");
        String host = problem.optString("host", "");
        Classification classification = mapClassification(problem);
        Instant timestamp = Instant.ofEpochSecond(problem.optLong("clock", Instant.now().getEpochSecond()));
        java.util.HashMap<String, String> customFields = new java.util.HashMap<>();
        for (String key : problem.keySet()) {
            customFields.put(key, problem.optString(key, ""));
        }
        return new ZabbixMessage(id, title, msg, host, classification, timestamp, customFields);
    }
}
