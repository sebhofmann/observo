package de.paschty.observo.di;

import com.google.inject.AbstractModule;
import com.google.inject.Singleton;
import de.paschty.observo.AppSettings;
import de.paschty.observo.LanguageManager;
import de.paschty.observo.SettingsManager;
import de.paschty.observo.FXMLLoaderFactory;
import de.paschty.observo.monitor.Server;
import de.paschty.observo.monitor.zabbix.ZabbixServer;

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
    bind(ZabbixServer.class).in(Singleton.class);
    bind(Server.class).to(ZabbixServer.class);
  }
}
