package de.paschty.observo.monitor;

public sealed interface ConfigurationValue<T> permits NumberField, TextField, PasswordField,
    BooleanField {

  T getValue();

  void setValue(T value);

  String getKey();

  default String getHelpKey() {
    return null;
  }
}
