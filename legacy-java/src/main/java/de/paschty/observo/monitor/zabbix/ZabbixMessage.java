package de.paschty.observo.monitor.zabbix;

import de.paschty.observo.monitor.Classification;
import de.paschty.observo.monitor.Message;
import java.time.Instant;
import java.util.HashMap;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONObject;

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

    static Classification mapSeverity(int severity, boolean acknowledged, boolean suppressed, boolean recovered) {
        if (recovered) return Classification.RECOVERY;
        if (acknowledged) return Classification.ACKNOWLEDGED;
        if (suppressed) return Classification.INFO;
        return switch (severity) {
            case 0 -> Classification.UNKNOWN;
            case 1, 2, 3 -> Classification.WARNING;
            case 4, 5 -> Classification.CRITICAL;
            default -> Classification.UNKNOWN;
        };
    }

    public static ZabbixMessage fromTrigger(JSONObject trigger) {
        JSONObject lastEvent = trigger.optJSONObject("lastEvent");
        if (lastEvent == null) {
            lastEvent = new JSONObject();
        }
        String triggerId = trigger.optString("triggerid", "");
        String eventId = lastEvent.optString("eventid", "");
        String triggerDescription = trigger.optString("description", "");
        String eventName = lastEvent.optString("name", triggerDescription);
        String opdata = lastEvent.optString("opdata", "");
        String title = (eventName == null || eventName.isEmpty()) ? triggerDescription : eventName;

        String statusInformation = buildStatusInformation(trigger, lastEvent, opdata);

        String host = "";
        boolean inMaintenance = false;
        JSONArray hosts = trigger.optJSONArray("hosts");
        if (hosts != null && hosts.length() > 0) {
            JSONObject firstHost = hosts.optJSONObject(0);
            if (firstHost != null) {
                host = firstHost.optString("name", firstHost.optString("host", ""));
            }
            for (int i = 0; i < hosts.length(); i++) {
                JSONObject h = hosts.optJSONObject(i);
                if (h != null && "1".equals(h.optString("maintenance_status", "0"))) {
                    inMaintenance = true;
                    break;
                }
            }
        }

        int severity = lastEvent.optInt("severity", trigger.optInt("priority", 0));
        boolean acknowledged = "1".equals(lastEvent.optString("acknowledged", "0"));
        boolean recovered = "0".equals(lastEvent.optString("value", "1"));

        Classification classification = mapSeverity(severity, acknowledged, inMaintenance, recovered);

        long clock = lastEvent.optLong("clock", 0L);
        Instant timestamp = clock > 0 ? Instant.ofEpochSecond(clock) : Instant.now();

        Map<String, String> custom = new HashMap<>();
        custom.put("triggerid", triggerId);
        custom.put("eventid", eventId);
        custom.put("severity", String.valueOf(severity));
        custom.put("acknowledged", acknowledged ? "1" : "0");
        custom.put("manual_close", trigger.optString("manual_close", "0"));
        if (inMaintenance) {
            custom.put("maintenance", "1");
        }

        return new ZabbixMessage(eventId.isEmpty() ? triggerId : eventId, title, statusInformation, host, classification, timestamp, custom);
    }

    private static String buildStatusInformation(JSONObject trigger, JSONObject lastEvent, String opdata) {
        if (opdata != null && !opdata.isEmpty()) {
            String name = lastEvent.optString("name", trigger.optString("description", ""));
            return name + " (" + opdata + ")";
        }
        JSONArray items = trigger.optJSONArray("items");
        if (items == null || items.length() == 0) {
            return lastEvent.optString("name", trigger.optString("description", ""));
        }
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < items.length(); i++) {
            JSONObject item = items.optJSONObject(i);
            if (item == null) continue;
            String name = item.optString("name", "");
            String lastvalue = item.optString("lastvalue", "");
            if (sb.length() > 0) sb.append(", ");
            sb.append(name).append(": ").append(lastvalue);
        }
        return sb.toString();
    }
}
