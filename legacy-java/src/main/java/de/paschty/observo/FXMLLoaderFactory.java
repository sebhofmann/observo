package de.paschty.observo;

import com.google.inject.Inject;
import com.google.inject.Injector;
import java.net.URL;
import java.util.ResourceBundle;
import javafx.fxml.FXMLLoader;

/**
 * Configures {@link FXMLLoader} instances so that controllers are created through Guice.
 */
public class FXMLLoaderFactory {

    private final Injector injector;
    private final I18N i18n;

    @Inject
    public FXMLLoaderFactory(Injector injector, I18N i18n) {
        this.injector = injector;
        this.i18n = i18n;
    }

    public FXMLLoader create(URL resource, ResourceBundle bundle) {
        FXMLLoader loader = new FXMLLoader(resource, bundle);
        loader.setControllerFactory(injector::getInstance);
        return loader;
    }

    public FXMLLoader create(URL resource) {
        return create(resource, i18n.getBundle());
    }
}
