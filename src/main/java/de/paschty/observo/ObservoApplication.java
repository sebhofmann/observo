package de.paschty.observo;

import com.google.inject.Guice;
import com.google.inject.Injector;
import de.paschty.observo.di.ObservoModule;
import java.io.IOException;
import java.io.InputStream;
import java.util.Locale;
import java.util.ResourceBundle;
import javafx.application.Application;
import javafx.fxml.FXMLLoader;
import javafx.scene.Parent;
import javafx.scene.Scene;
import javafx.scene.image.Image;
import javafx.stage.Stage;

public class ObservoApplication extends Application {

  @Override
  public void start(Stage stage) throws IOException {
    Injector injector = Guice.createInjector(new ObservoModule());

    SettingsManager settingsManager = injector.getInstance(SettingsManager.class);
    settingsManager.load();

    AppSettings appSettings = injector.getInstance(AppSettings.class);
    LanguageManager languageManager = injector.getInstance(LanguageManager.class);
    Locale locale = languageManager.getLocale();
    ResourceBundle bundle = ResourceBundle.getBundle("de.paschty.observo.messages", locale);
    FXMLLoaderFactory fxmlLoaderFactory = injector.getInstance(FXMLLoaderFactory.class);
    FXMLLoader fxmlLoader = fxmlLoaderFactory.create(ObservoApplication.class.getResource("main-view.fxml"), bundle);
    Parent root = fxmlLoader.load();
    Scene scene = new Scene(root, appSettings.getWindowWidth(), appSettings.getWindowHeight());
    stage.setTitle(bundle.getString("main.title"));
    stage.setScene(scene);

    try (InputStream is = getClass().getClassLoader().getResourceAsStream("observo.png")) {
      stage.getIcons().add(new Image(is));
    }
    if (appSettings.getWindowX() >= 0 && appSettings.getWindowY() >= 0) {
      stage.setX(appSettings.getWindowX());
      stage.setY(appSettings.getWindowY());
    }
    stage.show();

    stage.setOnCloseRequest(event -> {
      appSettings.setWindowX(stage.getX());
      appSettings.setWindowY(stage.getY());
      appSettings.setWindowWidth(stage.getWidth());
      appSettings.setWindowHeight(stage.getHeight());
      settingsManager.save();
      event.consume();
      stage.setIconified(true);
    });
  }
}
