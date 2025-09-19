package de.paschty.observo;

import static java.nio.file.StandardOpenOption.CREATE;
import static java.nio.file.StandardOpenOption.TRUNCATE_EXISTING;

import com.google.inject.Inject;
import de.paschty.observo.monitor.Configuration;
import de.paschty.observo.monitor.ConfigurationValue;
import java.io.*;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Locale;
import java.util.Properties;

public class SettingsManager {

  private static final Path CONFIG_DIR = Path.of(System.getProperty("user.home"))
      .resolve(".config/")
      .resolve("observo/");
  private static final Path CONFIG_FILE = CONFIG_DIR.resolve("observo.properties");

    private final AppSettings appSettings;
    private final LanguageManager languageManager;

    @Inject
    public SettingsManager(AppSettings appSettings, LanguageManager languageManager) {
        this.appSettings = appSettings;
        this.languageManager = languageManager;
    }

    public void load() {
        Properties props = new Properties();
        if (Files.exists(CONFIG_FILE)) {
            try (InputStream in = Files.newInputStream(CONFIG_FILE)) {
                props.load(in);
                String lang = props.getProperty("language", Locale.getDefault().getLanguage());
                languageManager.setLocale(new Locale(lang));
                appSettings.setWindowX(Double.parseDouble(props.getProperty("window.x", "-1")));
                appSettings.setWindowY(Double.parseDouble(props.getProperty("window.y", "-1")));
                appSettings.setWindowWidth(Double.parseDouble(props.getProperty("window.width", "1000")));
                appSettings.setWindowHeight(Double.parseDouble(props.getProperty("window.height", "400")));
                // Server-Konfiguration laden
                // Beispiel für ZabbixServerConfiguration
                de.paschty.observo.monitor.zabbix.ZabbixServerConfiguration config = new de.paschty.observo.monitor.zabbix.ZabbixServerConfiguration();
                for (var value : config.getValues()) {
                    String v = props.getProperty(value.getKey());
                    if (v != null) {
                        if (value instanceof de.paschty.observo.monitor.TextField tf) {
                            tf.setValue(v);
                        } else if (value instanceof de.paschty.observo.monitor.PasswordField pf) {
                            pf.setValue(v);
                        } else if (value instanceof de.paschty.observo.monitor.NumberField nf) {
                            try { nf.setValue(Integer.parseInt(v)); } catch (NumberFormatException ignored) {}
                        } else if (value instanceof de.paschty.observo.monitor.BooleanField bf) {
                            bf.setValue(Boolean.parseBoolean(v));
                        }
                    }
                }
                appSettings.setServerConfiguration(config);
            } catch (IOException e) {
                e.printStackTrace();
            }
        } else {
            // Defaults setzen
            languageManager.setLocale(Locale.getDefault());
            appSettings.setServerConfiguration(new de.paschty.observo.monitor.zabbix.ZabbixServerConfiguration());
        }
    }

    public void save() {
        Properties props = new Properties();
        props.setProperty("language", languageManager.getLocale().getLanguage());
        props.setProperty("window.x", Double.toString(appSettings.getWindowX()));
        props.setProperty("window.y", Double.toString(appSettings.getWindowY()));
        props.setProperty("window.width", Double.toString(appSettings.getWindowWidth()));
        props.setProperty("window.height", Double.toString(appSettings.getWindowHeight()));
        Configuration config = appSettings.getServerConfiguration();
        if (config != null) {
            for (ConfigurationValue<?> value : config.getValues()) {
                Object v = value.getValue();
                props.setProperty(value.getKey(), v != null ? v.toString() : "");
            }
        }
        try {
            Files.createDirectories(CONFIG_DIR);
            try (OutputStream out = Files.newOutputStream(CONFIG_FILE, CREATE, TRUNCATE_EXISTING)) {
                props.store(out, "observo Einstellungen");
            }
        } catch (IOException e) {
            e.printStackTrace();
        }
    }
}
