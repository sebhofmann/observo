package de.paschty.observo.monitor;

import java.util.List;

public interface Server {

  List<Message> pollMessages();

  Configuration getConfiguration();
  void setConfiguration(Configuration configuration);

  ConfigurationTestResult testConfiguration(Configuration configuration);

  /**
   * Acknowledge a message/event by its ID with an optional message.
   * @param eventId The event/message ID to acknowledge.
   * @param message Message to include in the acknowledgement (nullable).
   * @return true if acknowledgement was successful, false otherwise.
   */
  boolean acknowledgeMessage(String eventId, String message);

  public record ConfigurationTestResult(Boolean valid, String message){};
}
