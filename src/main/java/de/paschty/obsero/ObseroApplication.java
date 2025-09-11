package de.paschty.obsero;

import java.io.InputStream;
import javafx.application.Application;
import javafx.fxml.FXMLLoader;
import javafx.scene.Parent;
import javafx.scene.Scene;
import javafx.scene.image.Image;
import javafx.stage.Stage;

import java.io.IOException;

public class ObseroApplication extends Application {

  @Override
  public void start(Stage stage) throws IOException {
    // Einstellungen laden und anwenden
    SettingsManager.load();
    AppSettings appSettings = AppSettings.getInstance();
    // Sprache aus globalem Objekt setzen
    java.util.Locale locale = appSettings.getLocale();
    java.util.ResourceBundle bundle = java.util.ResourceBundle.getBundle(
        "de.paschty.obsero.messages", locale);
    FXMLLoader fxmlLoader = new FXMLLoader(ObseroApplication.class.getResource("main-view.fxml"),
        bundle);
    Parent root = fxmlLoader.load();
    Scene scene = new Scene(root, appSettings.getWindowWidth(), appSettings.getWindowHeight());
    stage.setTitle(bundle.getString("main.title"));
    stage.setScene(scene);
    // Icon setzen
    try (InputStream is = getClass().getClassLoader().getResourceAsStream("obsero.png")) {
      stage.getIcons().add(new Image(is));
    }
    if (appSettings.getWindowX() >= 0 && appSettings.getWindowY() >= 0) {
      stage.setX(appSettings.getWindowX());
      stage.setY(appSettings.getWindowY());
    }
    stage.show();
    // MainController holen
    MainController mainController = fxmlLoader.getController();

    // Listener zum Speichern der Fensterposition und -größe beim Schließen
    stage.setOnCloseRequest(event -> {
      appSettings.setWindowX(stage.getX());
      appSettings.setWindowY(stage.getY());
      appSettings.setWindowWidth(stage.getWidth());
      appSettings.setWindowHeight(stage.getHeight());
      SettingsManager.save();
      // Hintergrund-Thread beenden
      if (mainController != null) {
        mainController.stopPolling();
      }
      // Fenster minimieren statt beenden
      event.consume();
      stage.setIconified(true);
    });
  }
}
