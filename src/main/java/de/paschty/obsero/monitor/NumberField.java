package de.paschty.obsero.monitor;

public final class NumberField implements ConfigurationValue<Integer> {

  private Integer value;
  private String key;
  private final String helpKey;

  public NumberField(String key, Integer defaultValue) {
    this(key, defaultValue, null);
  }

  public NumberField(String key, Integer defaultValue, String helpKey) {
    this.value = defaultValue;
    this.key = key;
    this.helpKey = helpKey;
  }

  public NumberField(String key) {
    this(key, 0, null);
  }

  @Override
  public Integer getValue() {
    return value;
  }

  @Override
  public void setValue(Integer value) {
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
