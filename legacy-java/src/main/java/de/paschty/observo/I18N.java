package de.paschty.observo;

import com.google.inject.Inject;
import com.google.inject.Singleton;
import java.text.MessageFormat;
import java.util.Locale;
import java.util.MissingResourceException;
import java.util.ResourceBundle;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;

/**
 * Centralizes access to translatable resources and hides direct ResourceBundle usage.
 */
@Singleton
public class I18N {

  private static final Logger logger = LogManager.getLogger(I18N.class);
  private static final String BASE_NAME = "de.paschty.observo.messages";

  private final LanguageManager languageManager;

  private Locale cachedLocale;
  private ResourceBundle cachedBundle;

  @Inject
  public I18N(LanguageManager languageManager) {
    this.languageManager = languageManager;
  }

  public synchronized ResourceBundle getBundle() {
    Locale locale = languageManager.getLocale();
    if (cachedBundle == null || cachedLocale == null || !cachedLocale.equals(locale)) {
      cachedBundle = ResourceBundle.getBundle(BASE_NAME, locale);
      cachedLocale = locale;
    }
    return cachedBundle;
  }

  public String get(String key) {
    try {
      return getBundle().getString(key);
    } catch (MissingResourceException ex) {
      logger.warn("Missing i18n key: {}", key);
      return key;
    }
  }

  public boolean hasKey(String key) {
    return getBundle().containsKey(key);
  }

  public String format(String key, Object... args) {
    return MessageFormat.format(get(key), args);
  }

  public Locale getLocale() {
    return languageManager.getLocale();
  }

  public void setLocale(Locale locale) {
    languageManager.setLocale(locale);
    resetCache();
  }

  public synchronized void resetCache() {
    cachedBundle = null;
    cachedLocale = null;
  }
}
