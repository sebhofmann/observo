package de.paschty.observo;

import com.google.inject.Inject;
import com.google.inject.Injector;
import javafx.fxml.FXMLLoader;
import java.net.URL;
import java.util.ResourceBundle;

/**
 * Configures {@link FXMLLoader} instances so that controllers are created through Guice.
 */
public class FXMLLoaderFactory {

    private final Injector injector;

    @Inject
    public FXMLLoaderFactory(Injector injector) {
        this.injector = injector;
    }

    public FXMLLoader create(URL resource, ResourceBundle bundle) {
        FXMLLoader loader = new FXMLLoader(resource, bundle);
        loader.setControllerFactory(injector::getInstance);
        return loader;
    }

    public FXMLLoader create(URL resource) {
        return create(resource, null);
    }
}

