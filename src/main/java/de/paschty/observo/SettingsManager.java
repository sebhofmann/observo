package de.paschty.observo;

import static java.nio.file.StandardOpenOption.CREATE;
import static java.nio.file.StandardOpenOption.TRUNCATE_EXISTING;

import com.google.inject.Inject;
import de.paschty.observo.monitor.Configuration;
import de.paschty.observo.monitor.ConfigurationValue;
import de.paschty.observo.monitor.ServerProvider;
import java.io.*;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;
import java.util.Properties;
import java.util.Set;

public class SettingsManager {

  private static final Path CONFIG_DIR = Path.of(System.getProperty("user.home"))
      .resolve(".config/")
      .resolve("observo/");
  private static final Path CONFIG_FILE = CONFIG_DIR.resolve("observo.properties");

    private final AppSettings appSettings;
    private final LanguageManager languageManager;
    private final java.util.List<ServerProvider> serverProviders;
    private final Map<String, ServerProvider> providersById;

    @Inject
    public SettingsManager(AppSettings appSettings,
                           LanguageManager languageManager,
                           Set<ServerProvider> serverProviders) {
        this.appSettings = appSettings;
        this.languageManager = languageManager;
        this.serverProviders = new ArrayList<>(serverProviders);
        this.providersById = new LinkedHashMap<>();
        for (ServerProvider provider : this.serverProviders) {
            this.providersById.put(provider.id(), provider);
        }
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
                loadServerConfiguration(props);
            } catch (IOException e) {
                e.printStackTrace();
            }
        } else {
            // Defaults setzen
            languageManager.setLocale(Locale.getDefault());
            initialiseDefaultServerConfiguration();
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
        String providerId = appSettings.getServerProviderId();
        if (providerId != null) {
            props.setProperty("server.type", providerId);
        }
        if (config != null) {
            for (ConfigurationValue<?> value : config.getValues()) {
                Object v = value.getValue();
                String key = buildConfigKey(providerId, value.getKey());
                props.setProperty(key, v != null ? v.toString() : "");
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

    private void loadServerConfiguration(Properties props) {
        ServerProvider provider = resolveProvider(props.getProperty("server.type"));
        Configuration configuration = provider.createDefaultConfiguration();
        for (ConfigurationValue<?> value : configuration.getValues()) {
            String key = buildConfigKey(provider.id(), value.getKey());
            String storedValue = props.getProperty(key);
            if (storedValue == null) {
                // Backwards compatibility with legacy keys without prefix
                storedValue = props.getProperty(value.getKey());
            }
            if (storedValue != null) {
                applyValue(value, storedValue);
            }
        }
        appSettings.setServerProviderId(provider.id());
        appSettings.setServerConfiguration(configuration);
    }

    private void initialiseDefaultServerConfiguration() {
        ServerProvider provider = serverProviders.isEmpty()
            ? null
            : serverProviders.getFirst();
        if (provider == null) {
            throw new IllegalStateException("No ServerProvider registered");
        }
        appSettings.setServerProviderId(provider.id());
        appSettings.setServerConfiguration(provider.createDefaultConfiguration());
    }

    private ServerProvider resolveProvider(String providerId) {
        if (providerId != null) {
            ServerProvider provider = providersById.get(providerId);
            if (provider != null) {
                return provider;
            }
        }
        if (serverProviders.isEmpty()) {
            throw new IllegalStateException("No ServerProvider registered");
        }
        return serverProviders.getFirst();
    }

    private static void applyValue(ConfigurationValue<?> value, String storedValue) {
        try {
            if (value instanceof de.paschty.observo.monitor.TextField tf) {
                tf.setValue(storedValue);
            } else if (value instanceof de.paschty.observo.monitor.PasswordField pf) {
                pf.setValue(storedValue);
            } else if (value instanceof de.paschty.observo.monitor.NumberField nf) {
                nf.setValue(Integer.parseInt(storedValue));
            } else if (value instanceof de.paschty.observo.monitor.BooleanField bf) {
                bf.setValue(Boolean.parseBoolean(storedValue));
            }
        } catch (NumberFormatException ignored) {
            // Keep default if parsing fails
        }
    }

    private static String buildConfigKey(String providerId, String valueKey) {
        if (providerId == null || providerId.isBlank()) {
            return valueKey;
        }
        return "server." + providerId + "." + valueKey;
    }
}
