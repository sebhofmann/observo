package de.paschty.obsero.monitor;

public final class TextField implements ConfigurationValue<String> {

  private final String key;
  private String value;
  private final String helpKey;

  public TextField(String key, String defaultValue) {
    this(key, defaultValue, null);
  }

  public TextField(String key, String defaultValue, String helpKey) {
    this.value = defaultValue;
    this.key = key;
    this.helpKey = helpKey;
  }

  @Override
  public String getValue() {
    return value;
  }

  @Override
  public void setValue(String value) {
    this.value = value;
  }

  @Override
  public String getKey() {
    return key;
  }

  @Override
  public String getHelpKey() {
    return helpKey;
  }
}
