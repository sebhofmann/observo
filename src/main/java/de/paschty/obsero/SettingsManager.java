package de.paschty.obsero;

import de.paschty.obsero.monitor.Configuration;
import de.paschty.obsero.monitor.ConfigurationValue;
import java.io.*;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Locale;
import java.util.Properties;

public class SettingsManager {
    private static final String CONFIG_DIR = System.getProperty("user.home") + "/.config/obsero";
    private static final String CONFIG_FILE = CONFIG_DIR + "/obsero.properties";

    public static void load() {
        Properties props = new Properties();
        File file = new File(CONFIG_FILE);
        AppSettings settings = AppSettings.getInstance();
        if (file.exists()) {
            try (InputStream in = new FileInputStream(file)) {
                props.load(in);
                String lang = props.getProperty("language", Locale.getDefault().getLanguage());
                settings.setLocale(new Locale(lang));
                settings.setWindowX(Double.parseDouble(props.getProperty("window.x", "-1")));
                settings.setWindowY(Double.parseDouble(props.getProperty("window.y", "-1")));
                settings.setWindowWidth(Double.parseDouble(props.getProperty("window.width", "1000")));
                settings.setWindowHeight(Double.parseDouble(props.getProperty("window.height", "400")));
                // Server-Konfiguration laden
                // Beispiel für ZabbixServerConfiguration
                de.paschty.obsero.monitor.zabbix.ZabbixServerConfiguration config = new de.paschty.obsero.monitor.zabbix.ZabbixServerConfiguration();
                for (var value : config.getValues()) {
                    String v = props.getProperty(value.getKey());
                    if (v != null) {
                        if (value instanceof de.paschty.obsero.monitor.TextField tf) {
                            tf.setValue(v);
                        } else if (value instanceof de.paschty.obsero.monitor.PasswordField pf) {
                            pf.setValue(v);
                        } else if (value instanceof de.paschty.obsero.monitor.NumberField nf) {
                            try { nf.setValue(Integer.parseInt(v)); } catch (NumberFormatException ignored) {}
                        } else if (value instanceof de.paschty.obsero.monitor.BooleanField bf) {
                            bf.setValue(Boolean.parseBoolean(v));
                        }
                    }
                }
                settings.setServerConfiguration(config);
            } catch (IOException e) {
                e.printStackTrace();
            }
        } else {
            // Defaults setzen
            settings.setLocale(Locale.getDefault());
            settings.setServerConfiguration(new de.paschty.obsero.monitor.zabbix.ZabbixServerConfiguration());
        }
    }

    public static void save() {
        AppSettings settings = AppSettings.getInstance();
        Properties props = new Properties();
        props.setProperty("language", settings.getLocale().getLanguage());
        props.setProperty("window.x", Double.toString(settings.getWindowX()));
        props.setProperty("window.y", Double.toString(settings.getWindowY()));
        props.setProperty("window.width", Double.toString(settings.getWindowWidth()));
        props.setProperty("window.height", Double.toString(settings.getWindowHeight()));
        Configuration config = settings.getServerConfiguration();
        if (config != null) {
            for (ConfigurationValue<?> value : config.getValues()) {
                Object v = value.getValue();
                props.setProperty(value.getKey(), v != null ? v.toString() : "");
            }
        }
        try {
            Files.createDirectories(Path.of(CONFIG_DIR));
            try (OutputStream out = new FileOutputStream(CONFIG_FILE)) {
                props.store(out, "Obsero Einstellungen");
            }
        } catch (IOException e) {
            e.printStackTrace();
        }
    }
}
