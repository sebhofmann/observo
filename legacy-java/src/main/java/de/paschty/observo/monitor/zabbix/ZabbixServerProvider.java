package de.paschty.observo.monitor.zabbix;

import com.google.inject.Inject;
import com.google.inject.Provider;
import de.paschty.observo.monitor.Configuration;
import de.paschty.observo.monitor.Server;
import de.paschty.observo.monitor.ServerProvider;

public class ZabbixServerProvider implements ServerProvider {

  private final Provider<ZabbixServer> serverProvider;

  @Inject
  public ZabbixServerProvider(Provider<ZabbixServer> serverProvider) {
    this.serverProvider = serverProvider;
  }

  @Override
  public String id() {
    return "zabbix";
  }

  @Override
  public String displayI18nKey() {
    return "serverType.zabbix";
  }

  @Override
  public Class<? extends Configuration> configurationType() {
    return ZabbixServerConfiguration.class;
  }

  @Override
  public Configuration createDefaultConfiguration() {
    return new ZabbixServerConfiguration();
  }

  @Override
  public Server createServer() {
    return serverProvider.get();
  }
}
