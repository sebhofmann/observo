package de.paschty.observo.monitor;

import com.google.inject.Inject;
import com.google.inject.Singleton;
import de.paschty.observo.AppSettings;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;

/**
 * Manages the available {@link ServerProvider} implementations and the currently active server.
 */
@Singleton
public class ServerManager {

  private final List<ServerProvider> providers;
  private final Map<String, ServerProvider> providersById;
  private final Map<String, Configuration> configurations = new HashMap<>();
  private final AppSettings appSettings;

  private Server activeServer;
  private String activeProviderId;

  @Inject
  public ServerManager(Set<ServerProvider> providers, AppSettings appSettings) {
    if (providers.isEmpty()) {
      throw new IllegalStateException("At least one ServerProvider must be registered");
    }
    this.appSettings = appSettings;
    this.providers = new ArrayList<>(providers);
    this.providersById = new LinkedHashMap<>();
    for (ServerProvider provider : this.providers) {
      if (providersById.put(provider.id(), provider) != null) {
        throw new IllegalStateException("Duplicate ServerProvider id: " + provider.id());
      }
    }
    initialiseFromSettings();
  }

  private void initialiseFromSettings() {
    String configuredProviderId = appSettings.getServerProviderId();
    ServerProvider provider = providersById.get(configuredProviderId);
    if (provider == null) {
      provider = providers.getFirst();
    }
    this.activeProviderId = provider.id();

    Configuration configuration = appSettings.getServerConfiguration();
    if (configuration == null || !provider.configurationType().isInstance(configuration)) {
      configuration = provider.createDefaultConfiguration();
    }
    configurations.put(provider.id(), configuration);
    appSettings.setServerProviderId(provider.id());
    appSettings.setServerConfiguration(configuration);
  }

  public List<ServerProvider> getProviders() {
    return List.copyOf(providers);
  }

  public ServerProvider getProvider(String id) {
    ServerProvider provider = providersById.get(id);
    if (provider == null) {
      throw new IllegalArgumentException("Unknown ServerProvider id: " + id);
    }
    return provider;
  }

  public ServerProvider getActiveProvider() {
    ServerProvider provider = providersById.get(activeProviderId);
    if (provider == null) {
      provider = providers.getFirst();
      activeProviderId = provider.id();
    }
    return provider;
  }

  public Configuration getConfiguration(String providerId) {
    Configuration configuration = configurations.get(providerId);
    if (configuration != null) {
      return configuration;
    }
    ServerProvider provider = getProvider(providerId);
    Configuration created = provider.createDefaultConfiguration();
    configurations.put(providerId, created);
    return created;
  }

  public Configuration getActiveConfiguration() {
    return getConfiguration(activeProviderId);
  }

  public synchronized Server getActiveServer() {
    if (activeServer == null) {
      activate(activeProviderId);
    }
    return activeServer;
  }

  public synchronized Server activate(String providerId) {
    Objects.requireNonNull(providerId, "providerId");
    Configuration configuration = getConfiguration(providerId);
    ServerProvider provider = getProvider(providerId);

    if (providerId.equals(activeProviderId) && activeServer != null) {
      configurations.put(providerId, configuration);
      activeServer.setConfiguration(configuration);
      appSettings.setServerProviderId(provider.id());
      appSettings.setServerConfiguration(configuration);
      return activeServer;
    }

    Server server = provider.createServer();
    server.setConfiguration(configuration);
    configurations.put(providerId, configuration);
    this.activeServer = server;
    this.activeProviderId = provider.id();
    appSettings.setServerProviderId(provider.id());
    appSettings.setServerConfiguration(configuration);
    return server;
  }

  public void updateActiveConfiguration(Configuration configuration) {
    if (configuration == null) {
      return;
    }
    configurations.put(activeProviderId, configuration);
    appSettings.setServerConfiguration(configuration);
    if (activeServer != null) {
      activeServer.setConfiguration(configuration);
    }
  }
}
