package de.paschty.observo;

import com.google.inject.Inject;
import java.util.Locale;

public class LanguageManager {

    private final AppSettings appSettings;
    private Locale locale;

    @Inject
    public LanguageManager(AppSettings appSettings) {
        this.appSettings = appSettings;
        this.locale = appSettings.getLocale() != null ? appSettings.getLocale() : Locale.getDefault();
    }

    public Locale getLocale() {
        return locale;
    }

    public void setLocale(Locale newLocale) {
        this.locale = newLocale;
        appSettings.setLocale(newLocale);
    }
}
