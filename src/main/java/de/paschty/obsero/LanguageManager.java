package de.paschty.obsero;

import java.util.Locale;

public class LanguageManager {
    private static Locale locale = Locale.getDefault();

    public static Locale getLocale() {
        return locale;
    }

    public static void setLocale(Locale newLocale) {
        locale = newLocale;
    }
}

