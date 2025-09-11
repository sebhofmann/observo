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
import org.json.JSONArray;
import org.json.JSONObject;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;

public class ZabbixServer implements Server {

  private static final Logger logger = LogManager.getLogger(ZabbixServer.class);
  private ZabbixServerConfiguration configuration;
  private final HttpClient httpClient = HttpClient.newBuilder().build();

  public ZabbixServer() {
    this.configuration = new ZabbixServerConfiguration();
  }

  @Override
  public List<Message> pollMessages() {
    logger.debug("Starting pollMessages()");
    List<Message> messages = new ArrayList<>();
    try {
      String url = configuration.getUrl().getValue();
      String username = configuration.getUsername().getValue();
      String password = configuration.getPassword().getValue();
      String apiUrl = url + "/api_jsonrpc.php";
      logger.info("Logging in to Zabbix: {} as {}", url, username);
      // 1. Login: Get Auth-Token
      JSONObject loginRequest = new JSONObject();
      loginRequest.put("jsonrpc", "2.0");
      loginRequest.put("method", "user.login");
      loginRequest.put("params", new JSONObject()
        .put("user", username)
        .put("password", password));
      loginRequest.put("id", 1);
      HttpRequest request = HttpRequest.newBuilder()
        .uri(URI.create(apiUrl))
        .header("Content-Type", "application/json")
        .POST(HttpRequest.BodyPublishers.ofString(loginRequest.toString(), StandardCharsets.UTF_8))
        .build();
      HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
      logger.debug("Login response: {}", response.body());
      JSONObject loginResponse = new JSONObject(response.body());
      if (!loginResponse.has("result")) {
        logger.error("Login failed: {}", loginResponse.optString("error"));
        messages.add(createErrorMessage("Login failed: " + loginResponse.optString("error")));
        return messages;
      }
      String authToken = loginResponse.getString("result");
      logger.info("Login successful, token received");
      // 2. Get problems
      JSONObject problemRequest = new JSONObject();
      problemRequest.put("jsonrpc", "2.0");
      problemRequest.put("method", "problem.get");
      problemRequest.put("params", new JSONObject()
        .put("recent", true)
        .put("sortfield", "eventid")
        .put("sortorder", "DESC")
        .put("limit", 10));
      problemRequest.put("auth", authToken);
      problemRequest.put("id", 2);
      logger.debug("Sending problem request: {}", problemRequest);
      HttpRequest reqProblems = HttpRequest.newBuilder()
        .uri(URI.create(apiUrl))
        .header("Content-Type", "application/json")
        .POST(HttpRequest.BodyPublishers.ofString(problemRequest.toString(), StandardCharsets.UTF_8))
        .build();
      HttpResponse<String> respProblems = httpClient.send(reqProblems, HttpResponse.BodyHandlers.ofString());
      logger.debug("Problem response: {}", respProblems.body());
      JSONObject problemsResponse = new JSONObject(respProblems.body());
      if (!problemsResponse.has("result")) {
        logger.error("Error fetching problems: {}", problemsResponse.optString("error"));
        messages.add(createErrorMessage("Error fetching problems: " + problemsResponse.optString("error")));
        return messages;
      }
      JSONArray problems = problemsResponse.getJSONArray("result");
      logger.info("{} problems fetched", problems.length());
      String filterText = "";
      if (configuration.getFilter() != null && configuration.getFilter().getValue() != null) {
        filterText = configuration.getFilter().getValue().trim();
      }
      List<String> filterList = new ArrayList<>();
      if (!filterText.isEmpty()) {
        for (String s : filterText.split(";")) {
          String trimmed = s.trim();
          if (!trimmed.isEmpty()) filterList.add(trimmed);
        }
      }
      for (int i = 0; i < problems.length(); i++) {
        JSONObject problem = problems.getJSONObject(i);
        String name = problem.optString("name", "");
        boolean filtered = false;
        for (String filter : filterList) {
          if (!filter.isEmpty() && name.contains(filter)) {
            logger.info("Message '{}' filtered by filter '{}'.", name, filter);
            filtered = true;
            break;
          }
        }
        if (!filtered) {
          messages.add(ZabbixMessage.fromJson(problem));
        }
      }
    } catch (Exception e) {
      logger.error("Error in pollMessages(): {}", e.getMessage(), e);
      messages.add(createErrorMessage("Error: " + e.getMessage()));
    }
    return messages;
  }

  private Message createErrorMessage(String msg) {
    return new ZabbixMessage(
        "error", // ID für Fehlernachrichten
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
      logger.info("Configuration applied: {}", zabbixConfig);
    }
  }

  @Override
  public ConfigurationTestResult testConfiguration(Configuration configuration) {
    logger.debug("testConfiguration() called");
    if (!(configuration instanceof ZabbixServerConfiguration zabbixConfig)) {
      logger.error("Invalid configuration provided");
      return new ConfigurationTestResult(false, "Invalid configuration");
    }
    try {
      String url = zabbixConfig.getUrl().getValue();
      String username = zabbixConfig.getUsername().getValue();
      String password = zabbixConfig.getPassword().getValue();
      String apiUrl = url + "/api_jsonrpc.php";
      logger.info("Test login to Zabbix: {} as {}", url, username);
      JSONObject loginRequest = new JSONObject();
      loginRequest.put("jsonrpc", "2.0");
      loginRequest.put("method", "user.login");
      loginRequest.put("params", new JSONObject()
        .put("user", username)
        .put("password", password));
      loginRequest.put("id", 1);
      HttpRequest request = HttpRequest.newBuilder()
        .uri(URI.create(apiUrl))
        .header("Content-Type", "application/json")
        .POST(HttpRequest.BodyPublishers.ofString(loginRequest.toString(), StandardCharsets.UTF_8))
        .build();
      HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
      logger.debug("Test login response: {}", response.body());
      JSONObject loginResponse = new JSONObject(response.body());
      if (!loginResponse.has("result")) {
        logger.error("Test login failed: {}", loginResponse.optString("error"));
        return new ConfigurationTestResult(false, "Login failed: " + loginResponse.optString("error"));
      }
      logger.info("Test login successful");
      return new ConfigurationTestResult(true, "Connection successful");
    } catch (Exception e) {
      logger.error("Error in test login: {}", e.getMessage(), e);
      return new ConfigurationTestResult(false, "Error: " + e.getMessage());
    }
  }

  @Override
  public boolean acknowledgeMessage(String eventId, String message) {
    logger.info("Acknowledge event {} with message: {}", eventId, message);
    String url = configuration.getUrl().getValue();
    String username = configuration.getUsername().getValue();
    String password = configuration.getPassword().getValue();
    String apiUrl = url + "/api_jsonrpc.php";
    try {
      // 1. Login: Get Auth-Token
      JSONObject loginRequest = new JSONObject();
      loginRequest.put("jsonrpc", "2.0");
      loginRequest.put("method", "user.login");
      loginRequest.put("params", new JSONObject()
          .put("user", username)
          .put("password", password));
      loginRequest.put("id", 1);
      HttpRequest loginHttpRequest = HttpRequest.newBuilder()
          .uri(URI.create(apiUrl))
          .header("Content-Type", "application/json")
          .POST(HttpRequest.BodyPublishers.ofString(loginRequest.toString(), StandardCharsets.UTF_8))
          .build();
      HttpResponse<String> loginResponse = httpClient.send(loginHttpRequest, HttpResponse.BodyHandlers.ofString());
      JSONObject loginJson = new JSONObject(loginResponse.body());
      if (!loginJson.has("result")) {
        logger.error("Login failed: {}", loginJson);
        return false;
      }
      String authToken = loginJson.getString("result");
      // 2. Acknowledge event
      JSONObject ackRequest = new JSONObject();
      ackRequest.put("jsonrpc", "2.0");
      ackRequest.put("method", "event.acknowledge");
      JSONObject params = new JSONObject();
      params.put("eventids", eventId);
      params.put("action", 6);
      if (message != null && !message.isEmpty()) {
        params.put("message", message);
      }
      ackRequest.put("params", params);
      ackRequest.put("auth", authToken);
      ackRequest.put("id", 2);
      HttpRequest ackHttpRequest = HttpRequest.newBuilder()
          .uri(URI.create(apiUrl))
          .header("Content-Type", "application/json")
          .POST(HttpRequest.BodyPublishers.ofString(ackRequest.toString(), StandardCharsets.UTF_8))
          .build();
      HttpResponse<String> ackResponse = httpClient.send(ackHttpRequest, HttpResponse.BodyHandlers.ofString());
      logger.info("Acknowledge acknowledge response: {}", ackResponse.body());
      JSONObject ackJson = new JSONObject(ackResponse.body());
      if (ackJson.has("result") && ackJson.getJSONObject("result").has("eventids")) {
        JSONArray eventIds = ackJson.getJSONObject("result").getJSONArray("eventids");
        boolean success = false;
        for (int i = 0; i < eventIds.length(); i++) {
          if (String.valueOf(eventIds.get(i)).equals(eventId)) {
            success = true;
            break;
          }
        }
        return success;
      } else {
        logger.error("Acknowledge failed: {}", ackJson);
        return false;
      }
    } catch (Exception e) {
      logger.error("Exception during acknowledgeMessage", e);
      return false;
    }
  }

}
