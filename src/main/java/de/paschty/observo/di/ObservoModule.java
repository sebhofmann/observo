package de.paschty.observo.di;

import com.google.inject.AbstractModule;
import com.google.inject.Provides;
import com.google.inject.Singleton;
import de.paschty.observo.AppSettings;
import de.paschty.observo.LanguageManager;
import de.paschty.observo.SettingsManager;
import de.paschty.observo.FXMLLoaderFactory;
import de.paschty.observo.monitor.ServerProvider;
import de.paschty.observo.monitor.zabbix.ZabbixServerProvider;
import java.util.LinkedHashSet;
import java.util.Set;

/**
 * Central Guice module that wires the application services.
 */
public class ObservoModule extends AbstractModule {

  @Override
  protected void configure() {
    bind(AppSettings.class).in(Singleton.class);
    bind(SettingsManager.class).in(Singleton.class);
    bind(LanguageManager.class).in(Singleton.class);
    bind(FXMLLoaderFactory.class).in(Singleton.class);
  }

  @Provides
  @Singleton
  Set<ServerProvider> provideServerProviders(ZabbixServerProvider zabbixProvider) {
    var providers = new LinkedHashSet<ServerProvider>();
    providers.add(zabbixProvider);
    return providers;
  }
}
