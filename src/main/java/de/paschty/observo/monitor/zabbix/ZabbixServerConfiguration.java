package de.paschty.observo.monitor.zabbix;

import de.paschty.observo.monitor.Configuration;
import de.paschty.observo.monitor.ConfigurationValue;
import de.paschty.observo.monitor.PasswordField;
import de.paschty.observo.monitor.TextField;
import de.paschty.observo.monitor.NumberField;
import java.util.List;

public class ZabbixServerConfiguration implements Configuration {

  private TextField url;
  private TextField username;
  private PasswordField password;
  private NumberField pollIntervalMillis;
  private TextField filter;

  public ZabbixServerConfiguration() {
    this.url = new TextField("zabbix.url", "http://localhost/zabbix");
    this.username = new TextField("zabbix.username", "Admin");
    this.password = new PasswordField("zabbix.password", "zabbix");
    this.pollIntervalMillis = new NumberField("zabbix.pollIntervalMillis", 10000); // Default: 10 Sekunden
    this.filter = new TextField("zabbix.filter", "", "serverConfig.help.zabbix.filter");
  }

  public TextField getUrl() {
    return url;
  }

  public TextField getUsername() {
    return username;
  }

  public PasswordField getPassword() {
    return password;
  }

  public NumberField getPollIntervalMillis() {
    return pollIntervalMillis;
  }

  public TextField getFilter() {
    return filter;
  }

  @Override
  public List<ConfigurationValue> getValues() {
    return List.of(url, username, password, pollIntervalMillis, filter);
  }
}
