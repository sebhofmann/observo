package de.paschty.obsero.monitor;

public final class BooleanField implements ConfigurationValue<Boolean> {

  private final String key;
  private Boolean value;
  private final String helpKey;

  public BooleanField(String key, Boolean defaultValue) {
    this(key, defaultValue, null);
  }

  public BooleanField(String key, Boolean defaultValue, String helpKey) {
    this.value = defaultValue;
    this.key = key;
    this.helpKey = helpKey;
  }

  @Override
  public Boolean getValue() {
    return value;
  }

  @Override
  public void setValue(Boolean value) {
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
