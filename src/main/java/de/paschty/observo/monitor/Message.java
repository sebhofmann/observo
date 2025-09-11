package de.paschty.observo.monitor;

import java.time.Instant;
import java.util.Map;

public interface Message {

  String getId();
  String getTitle();
  String getMessage();
  String hostname();
  Classification getClassification();
  Instant getTimestamp();
  Map<String, String> getCustomFields();
}
