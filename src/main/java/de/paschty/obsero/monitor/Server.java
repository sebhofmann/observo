package de.paschty.obsero.monitor;

import java.util.List;

public interface Server {

  List<Message> pollMessages();

  Configuration getConfiguration();
  void setConfiguration(Configuration configuration);

  ConfigurationTestResult testConfiguration(Configuration configuration);

  public record ConfigurationTestResult(Boolean valid, String message){};
}
