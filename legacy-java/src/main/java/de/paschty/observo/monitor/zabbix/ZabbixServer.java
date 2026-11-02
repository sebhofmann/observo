package de.paschty.observo.monitor.zabbix;

import de.paschty.observo.monitor.Classification;
import de.paschty.observo.monitor.Configuration;
import de.paschty.observo.monitor.Message;
import de.paschty.observo.monitor.Server;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;
import org.json.JSONArray;
import org.json.JSONObject;

public class ZabbixServer implements Server {

  private static final Logger logger = LogManager.getLogger(ZabbixServer.class);
  private static final int CHUNK_SIZE = 200;
  private static final int ACK_ACTION_ACKNOWLEDGE = 2;
  private static final int ACK_ACTION_MESSAGE = 4;

  private ZabbixServerConfiguration configuration;
  private final HttpClient httpClient = HttpClient.newBuilder().build();

  // Cached session state — refreshed on demand
  private String apiVersion = "";
  private String authToken = "";
  private boolean useBearerHeader = false;

  public ZabbixServer() {
    this.configuration = new ZabbixServerConfiguration();
  }

  @Override
  public List<Message> pollMessages() {
    logger.debug("pollMessages() start");
    List<Message> messages = new ArrayList<>();
    try {
      ensureAuthenticated();
      List<JSONObject> triggers = fetchTriggers();
      logger.info("Fetched {} active triggers", triggers.size());
      List<String> filterList = parseFilter();
      for (JSONObject trigger : triggers) {
        if (isFiltered(trigger, filterList)) continue;
        messages.add(ZabbixMessage.fromTrigger(trigger));
      }
    } catch (ZabbixApiException e) {
      logger.error("API error in pollMessages: {}", e.getMessage());
      // Force re-auth on next call in case token expired
      authToken = "";
      messages.add(errorMessage(e.getMessage()));
    } catch (Exception e) {
      logger.error("Error in pollMessages(): {}", e.getMessage(), e);
      messages.add(errorMessage("Error: " + e.getMessage()));
    }
    return messages;
  }

  private List<String> parseFilter() {
    List<String> filterList = new ArrayList<>();
    if (configuration.getFilter() == null) return filterList;
    String filterText = configuration.getFilter().getValue();
    if (filterText == null) return filterList;
    for (String s : filterText.split(";")) {
      String trimmed = s.trim();
      if (!trimmed.isEmpty()) filterList.add(trimmed);
    }
    return filterList;
  }

  private boolean isFiltered(JSONObject trigger, List<String> filterList) {
    if (filterList.isEmpty()) return false;
    String description = trigger.optString("description", "");
    JSONObject lastEvent = trigger.optJSONObject("lastEvent");
    String eventName = lastEvent != null ? lastEvent.optString("name", "") : "";
    for (String filter : filterList) {
      if (description.contains(filter) || eventName.contains(filter)) {
        logger.info("Trigger '{}' filtered by '{}'", description, filter);
        return true;
      }
    }
    return false;
  }

  private List<JSONObject> fetchTriggers() throws Exception {
    JSONObject idsParams = new JSONObject()
        .put("only_true", true)
        .put("skipDependent", true)
        .put("monitored", true)
        .put("active", true)
        .put("output", new JSONArray().put("triggerid"));
    JSONObject idsResp = apiRequest("trigger.get", idsParams);
    JSONArray idResult = idsResp.getJSONArray("result");
    List<String> triggerIds = new ArrayList<>(idResult.length());
    for (int i = 0; i < idResult.length(); i++) {
      triggerIds.add(idResult.getJSONObject(i).getString("triggerid"));
    }

    List<JSONObject> triggers = new ArrayList<>(triggerIds.size());
    for (int i = 0; i < triggerIds.size(); i += CHUNK_SIZE) {
      List<String> chunk = triggerIds.subList(i, Math.min(i + CHUNK_SIZE, triggerIds.size()));
      JSONObject params = new JSONObject()
          .put("only_true", true)
          .put("skipDependent", true)
          .put("monitored", true)
          .put("active", true)
          .put("output", new JSONArray()
              .put("triggerid").put("description").put("lastchange").put("manual_close").put("priority"))
          .put("triggerids", new JSONArray(chunk))
          .put("selectLastEvent", new JSONArray()
              .put("eventid").put("name").put("opdata").put("clock").put("acknowledged").put("value").put("severity"))
          .put("selectHosts", new JSONArray()
              .put("hostid").put("host").put("name").put("status").put("maintenance_status"))
          .put("selectItems", new JSONArray()
              .put("name").put("lastvalue").put("state").put("lastclock"));
      JSONObject resp = apiRequest("trigger.get", params);
      JSONArray arr = resp.getJSONArray("result");
      for (int j = 0; j < arr.length(); j++) {
        triggers.add(arr.getJSONObject(j));
      }
    }
    return triggers;
  }

  private void ensureAuthenticated() throws Exception {
    if (apiVersion.isEmpty()) {
      apiVersion = fetchApiVersion();
      logger.info("Zabbix API version: {}", apiVersion);
    }
    if (!authToken.isEmpty()) return;
    String token = configuration.getApiToken() != null ? configuration.getApiToken().getValue() : "";
    if (token != null && !token.isEmpty()) {
      authToken = token.trim();
      useBearerHeader = true;
      logger.info("Using configured API token (Bearer auth)");
      return;
    }
    login();
  }

  private String fetchApiVersion() throws Exception {
    JSONObject body = new JSONObject()
        .put("jsonrpc", "2.0")
        .put("method", "apiinfo.version")
        .put("params", new JSONArray())
        .put("id", nextId());
    JSONObject resp = sendJsonRpc(body, false);
    return resp.getString("result");
  }

  private void login() throws Exception {
    String username = configuration.getUsername().getValue();
    String password = configuration.getPassword().getValue();
    String userKey = isAtLeast64() ? "username" : "user";
    JSONObject body = new JSONObject()
        .put("jsonrpc", "2.0")
        .put("method", "user.login")
        .put("params", new JSONObject().put(userKey, username).put("password", password))
        .put("id", nextId());
    JSONObject resp = sendJsonRpc(body, false);
    authToken = resp.getString("result");
    useBearerHeader = isAtLeast64();
    logger.info("Login successful (bearer={})", useBearerHeader);
  }

  private JSONObject apiRequest(String method, JSONObject params) throws Exception {
    JSONObject body = new JSONObject()
        .put("jsonrpc", "2.0")
        .put("method", method)
        .put("params", params)
        .put("id", nextId());
    if (!useBearerHeader && !authToken.isEmpty()) {
      body.put("auth", authToken);
    }
    return sendJsonRpc(body, useBearerHeader && !authToken.isEmpty());
  }

  private JSONObject sendJsonRpc(JSONObject body, boolean withBearer) throws Exception {
    String apiUrl = configuration.getUrl().getValue() + "/api_jsonrpc.php";
    HttpRequest.Builder builder = HttpRequest.newBuilder()
        .uri(URI.create(apiUrl))
        .header("Content-Type", "application/json-rpc")
        .header("Accept", "application/json")
        .POST(HttpRequest.BodyPublishers.ofString(body.toString(), StandardCharsets.UTF_8));
    if (withBearer) {
      builder.header("Authorization", "Bearer " + authToken);
    }
    HttpResponse<String> resp = httpClient.send(builder.build(), HttpResponse.BodyHandlers.ofString());
    if (resp.statusCode() / 100 != 2) {
      throw new ZabbixApiException("HTTP " + resp.statusCode() + " from Zabbix API");
    }
    JSONObject json = new JSONObject(resp.body());
    if (json.has("error")) {
      JSONObject err = json.getJSONObject("error");
      String msg = err.optString("data", err.optString("message", "Unknown API error"));
      throw new ZabbixApiException(msg);
    }
    if (!json.has("result")) {
      throw new ZabbixApiException("No result in response");
    }
    return json;
  }

  private long requestId = 0;

  private long nextId() {
    return ++requestId;
  }

  private boolean isAtLeast64() {
    return compareVersion(apiVersion, "6.4") >= 0;
  }

  static int compareVersion(String a, String b) {
    String[] aParts = a.split("\\.");
    String[] bParts = b.split("\\.");
    int len = Math.max(aParts.length, bParts.length);
    for (int i = 0; i < len; i++) {
      int ai = i < aParts.length ? parsePart(aParts[i]) : 0;
      int bi = i < bParts.length ? parsePart(bParts[i]) : 0;
      if (ai != bi) return Integer.compare(ai, bi);
    }
    return 0;
  }

  private static int parsePart(String s) {
    StringBuilder digits = new StringBuilder();
    for (char c : s.toCharArray()) {
      if (Character.isDigit(c)) digits.append(c);
      else break;
    }
    if (digits.length() == 0) return 0;
    try {
      return Integer.parseInt(digits.toString());
    } catch (NumberFormatException e) {
      return 0;
    }
  }

  private Message errorMessage(String msg) {
    return new ZabbixMessage(
        "error",
        "Zabbix Fehler",
        msg,
        "-",
        Classification.UNKNOWN,
        Instant.now(),
        Map.of()
    );
  }

  @Override
  public Configuration getConfiguration() {
    return configuration;
  }

  @Override
  public void setConfiguration(Configuration configuration) {
    logger.debug("setConfiguration() called");
    if (configuration instanceof ZabbixServerConfiguration zabbixConfig) {
      this.configuration = zabbixConfig;
      // Drop cached state so new credentials take effect
      this.apiVersion = "";
      this.authToken = "";
      this.useBearerHeader = false;
    }
  }

  @Override
  public ConfigurationTestResult testConfiguration(Configuration configuration) {
    logger.debug("testConfiguration() called");
    if (!(configuration instanceof ZabbixServerConfiguration zabbixConfig)) {
      return new ConfigurationTestResult(false, "Invalid configuration");
    }
    ZabbixServer probe = new ZabbixServer();
    probe.configuration = zabbixConfig;
    try {
      probe.ensureAuthenticated();
      // Cheap probe call to confirm token works
      probe.apiRequest("apiinfo.version", new JSONObject());
      return new ConfigurationTestResult(true, "Connection successful");
    } catch (ZabbixApiException e) {
      return new ConfigurationTestResult(false, "Login failed: " + e.getMessage());
    } catch (Exception e) {
      logger.error("Error in test login: {}", e.getMessage(), e);
      return new ConfigurationTestResult(false, "Error: " + e.getMessage());
    }
  }

  @Override
  public boolean acknowledgeMessage(String eventId, String message) {
    logger.info("Acknowledge event {} (message: {})", eventId, message);
    if (eventId == null || eventId.isEmpty() || "error".equals(eventId)) {
      return false;
    }
    try {
      ensureAuthenticated();
      int action = ACK_ACTION_ACKNOWLEDGE;
      JSONObject params = new JSONObject()
          .put("eventids", eventId);
      if (message != null && !message.isEmpty()) {
        action |= ACK_ACTION_MESSAGE;
        params.put("message", message);
      }
      params.put("action", action);
      JSONObject resp = apiRequest("event.acknowledge", params);
      JSONObject result = resp.optJSONObject("result");
      if (result == null) {
        logger.error("Acknowledge: missing result");
        return false;
      }
      JSONArray eventIds = result.optJSONArray("eventids");
      if (eventIds == null) return false;
      for (int i = 0; i < eventIds.length(); i++) {
        if (String.valueOf(eventIds.get(i)).equals(eventId)) return true;
      }
      return false;
    } catch (ZabbixApiException e) {
      logger.error("Acknowledge failed: {}", e.getMessage());
      authToken = "";
      return false;
    } catch (Exception e) {
      logger.error("Exception during acknowledgeMessage", e);
      return false;
    }
  }

  private static class ZabbixApiException extends RuntimeException {
    ZabbixApiException(String message) {
      super(message);
    }
  }
}
