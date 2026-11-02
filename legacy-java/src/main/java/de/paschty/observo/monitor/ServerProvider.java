package de.paschty.observo.monitor;

/**
 * Describes a pluggable server implementation that can be offered to the user.
 */
public interface ServerProvider {

  /**
   * Unique technical identifier for the provider. Persisted in settings.
   */
  String id();

  /**
   * Resource bundle key for the human readable label that is shown in the UI.
   */
  String displayI18nKey();

  /**
   * @return the configuration type that belongs to this provider.
   */
  Class<? extends Configuration> configurationType();

  /**
   * Creates a default configuration instance for the provider.
   */
  Configuration createDefaultConfiguration();

  /**
   * Creates a new server instance that can handle the configuration.
   */
  Server createServer();
}
