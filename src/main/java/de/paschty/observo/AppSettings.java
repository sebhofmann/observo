package de.paschty.observo;

import de.paschty.observo.monitor.Configuration;
import java.util.Locale;

public class AppSettings {
    private Locale locale;
    private Configuration serverConfiguration;

    private static AppSettings instance;

    private AppSettings() {}

    public static AppSettings getInstance() {
        if (instance == null) {
            instance = new AppSettings();
        }
        return instance;
    }

    public Locale getLocale() {
        return locale;
    }

    public void setLocale(Locale locale) {
        this.locale = locale;
    }

    public Configuration getServerConfiguration() {
        return serverConfiguration;
    }

    public void setServerConfiguration(Configuration config) {
        this.serverConfiguration = config;
    }

    private double windowX = -1;
    private double windowY = -1;
    private double windowWidth = 1000;
    private double windowHeight = 400;

    public double getWindowX() { return windowX; }
    public void setWindowX(double x) { this.windowX = x; }
    public double getWindowY() { return windowY; }
    public void setWindowY(double y) { this.windowY = y; }
    public double getWindowWidth() { return windowWidth; }
    public void setWindowWidth(double w) { this.windowWidth = w; }
    public double getWindowHeight() { return windowHeight; }
    public void setWindowHeight(double h) { this.windowHeight = h; }
}
