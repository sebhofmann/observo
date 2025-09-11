package de.paschty.obsero;

import de.paschty.obsero.monitor.Message;
import de.paschty.obsero.monitor.Server;
import de.paschty.obsero.monitor.zabbix.ZabbixServer;
import de.paschty.obsero.monitor.zabbix.ZabbixServerConfiguration;

import javafx.collections.FXCollections;
import javafx.collections.ObservableList;
import javafx.fxml.FXML;
import javafx.fxml.FXMLLoader;
import javafx.scene.Parent;
import javafx.scene.Scene;
import javafx.scene.control.Label;
import javafx.scene.control.TableColumn;
import javafx.scene.control.TableView;
import javafx.scene.layout.Region;
import javafx.scene.layout.VBox;
import javafx.stage.Stage;
import javafx.scene.media.AudioClip;
import javafx.application.Platform;

import java.io.IOException;
import java.util.List;
import java.util.ResourceBundle;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.TimeUnit;
import javafx.util.Duration;
import org.controlsfx.control.Notifications;

public class MainController {

  @FXML
  private Label welcomeText;

  @FXML
  private VBox rootVBox;

  @FXML
  private TableView<Message> messagesTable;
  @FXML
  private TableColumn<Message, String> titleColumn;
  @FXML
  private TableColumn<Message, String> messageColumn;
  @FXML
  private TableColumn<Message, String> hostColumn;
  @FXML
  private TableColumn<Message, String> classificationColumn;
  @FXML
  private TableColumn<Message, String> timestampColumn;
  @FXML
  private Label okLabel;

  private ScheduledExecutorService scheduler;
  private ScheduledFuture<?> pollTask;
  private int lastPollInterval = -1;
  private boolean hadMessages = false;

  @FXML
  protected void onHelloButtonClick() {
    welcomeText.setText("Welcome to JavaFX Application!");
  }

  @FXML
  protected void onServerConfigMenuClick() {
    try {
      // ResourceBundle für die aktuelle Sprache laden (hier: Deutsch, kann dynamisch gemacht werden)
      java.util.Locale locale = java.util.Locale.getDefault();
      ResourceBundle bundle = ResourceBundle.getBundle("de.paschty.obsero.messages", locale);
      FXMLLoader loader = new FXMLLoader(getClass().getResource("server-config-view.fxml"), bundle);
      Parent root = loader.load();
      ServerConfigController controller = loader.getController();
      controller.setConfiguration(AppSettings.getInstance().getServerConfiguration());
      Stage stage = new Stage();
      stage.setTitle(bundle.getString("serverConfig.title"));
      stage.setScene(new Scene(root));
      stage.showAndWait(); // Warten bis geschlossen
      // Nach Schließen: Timer neu starten, falls Intervall geändert
      restartPollingIfNeeded();
    } catch (IOException e) {
      e.printStackTrace();
    }
  }

  @FXML
  protected void onSettingsMenuClick() {
    try {
      java.util.Locale locale = LanguageManager.getLocale();
      ResourceBundle bundle = ResourceBundle.getBundle("de.paschty.obsero.messages", locale);
      FXMLLoader loader = new FXMLLoader(getClass().getResource("settings-view.fxml"), bundle);
      Parent root = loader.load();
      Stage stage = new Stage();
      stage.setTitle(bundle.getString("settings.title"));
      stage.setScene(new Scene(root));
      stage.showAndWait();
      // Nach Schließen des Dialogs: Hauptansicht neu laden, falls Sprache geändert wurde
      reloadMainView();
    } catch (IOException e) {
      e.printStackTrace();
    }
  }

  @FXML
  public void initialize() {
    // Spaltenzuordnung
    titleColumn.setCellValueFactory(cellData -> new javafx.beans.property.SimpleStringProperty(cellData.getValue().getTitle()));
    messageColumn.setCellValueFactory(cellData -> new javafx.beans.property.SimpleStringProperty(cellData.getValue().getMessage()));
    hostColumn.setCellValueFactory(cellData -> new javafx.beans.property.SimpleStringProperty(cellData.getValue().hostname()));
    classificationColumn.setCellValueFactory(cellData -> new javafx.beans.property.SimpleStringProperty(cellData.getValue().getClassification().name()));
    timestampColumn.setCellValueFactory(cellData -> new javafx.beans.property.SimpleStringProperty(
      java.time.format.DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm:ss").format(cellData.getValue().getTimestamp().atZone(java.time.ZoneId.systemDefault()))
    ));
    //loadMessages();
    startPolling();

  }

  private void startPolling() {
    stopPolling(); // Vorher alles stoppen
    scheduler = Executors.newSingleThreadScheduledExecutor();
    restartPollingIfNeeded();
  }

  private void restartPollingIfNeeded() {
    // Vor dem Erstellen eines neuen Schedulers immer den alten stoppen
    if (scheduler != null && !scheduler.isShutdown()) {
      scheduler.shutdownNow();
      scheduler = null;
    }
    scheduler = Executors.newSingleThreadScheduledExecutor();
    int interval = getPollInterval();
    if (interval <= 0) interval = 10000;
    if (pollTask != null) pollTask.cancel(false);
    pollTask = scheduler.scheduleAtFixedRate(this::loadMessages, 0, interval, TimeUnit.MILLISECONDS);
    lastPollInterval = interval;
  }

  private int getPollInterval() {
    var config = AppSettings.getInstance().getServerConfiguration();
    if (config instanceof ZabbixServerConfiguration zabbixConfig) {
      return zabbixConfig.getPollIntervalMillis().getValue();
    }
    return 10000;
  }

  private void loadMessages() {
    Server server = new ZabbixServer();
    // Konfiguration aus AppSettings übernehmen
    server.setConfiguration(AppSettings.getInstance().getServerConfiguration());
    List<Message> messages = server.pollMessages();
    boolean hasMessages = messages != null && !messages.isEmpty();
    // Benachrichtigung und Sound, wenn Wechsel von keine zu mindestens eine Nachricht
    if (!hadMessages && hasMessages) {
      ResourceBundle bundle = ResourceBundle.getBundle("de.paschty.obsero.messages", LanguageManager.getLocale());
      String newMessageTitle = bundle.getString("notification.newMessages.title");
      String newMessageText = bundle.getString("notification.newMessages.text");
      Platform.runLater(() -> {
        sendSystemNotification(newMessageTitle, newMessageText);
        playNotificationSound();
      });
    }
    hadMessages = hasMessages;
    Platform.runLater(() -> {
      ResourceBundle bundle = ResourceBundle.getBundle("de.paschty.obsero.messages", LanguageManager.getLocale());
      if (!hasMessages) {
        messagesTable.setVisible(false);
        okLabel.setVisible(true);
        okLabel.setText(bundle.getString("status.ok"));
        okLabel.setStyle("-fx-background-color: #4CAF50; -fx-text-fill: white; -fx-font-weight: bold; -fx-padding: 10px; -fx-font-size: 20px;");
        okLabel.setAlignment(javafx.geometry.Pos.CENTER);
        okLabel.setMaxWidth(Double.MAX_VALUE);
        okLabel.setMaxHeight(Double.MAX_VALUE);
        javafx.scene.layout.VBox.setVgrow(okLabel, javafx.scene.layout.Priority.ALWAYS);
        rootVBox.setAlignment(javafx.geometry.Pos.CENTER);
      } else {
        ObservableList<Message> data = FXCollections.observableArrayList(messages);
        messagesTable.setItems(data);
        messagesTable.setVisible(true);
        okLabel.setVisible(false);
        okLabel.setStyle(""); // Style zurücksetzen
        okLabel.setAlignment(javafx.geometry.Pos.BASELINE_LEFT);
        okLabel.setMaxWidth(Region.USE_COMPUTED_SIZE);
        okLabel.setMaxHeight(Region.USE_COMPUTED_SIZE);
        javafx.scene.layout.VBox.setVgrow(okLabel, javafx.scene.layout.Priority.NEVER);
        rootVBox.setAlignment(javafx.geometry.Pos.TOP_LEFT);
      }
    });
  }

  private void sendSystemNotification(String title, String message) {
    Notifications notification = Notifications
        .create()
        .title(title)
        .text(message)
        .hideAfter(Duration.minutes(1.0));
    if (rootVBox != null && rootVBox.getScene() != null && rootVBox.getScene().getWindow() != null) {
        notification.owner(rootVBox.getScene().getWindow());
        // Fenster in den Vordergrund holen
        javafx.stage.Window window = rootVBox.getScene().getWindow();
        if (window instanceof Stage stage) {
            stage.setIconified(false); // Falls minimiert, wiederherstellen
            stage.toFront();           // In den Vordergrund holen
            stage.requestFocus();      // Fokus setzen
        }
    }
    notification.show();
  }

  private void playNotificationSound() {
    try {
      AudioClip clip = new AudioClip(getClass().getClassLoader().getResource("bell.wav").toExternalForm());
      clip.play();
    } catch (Exception e) {
      // Ignorieren, falls nicht möglich
    }
  }

  private void reloadMainView() {
    try {
      java.util.Locale locale = LanguageManager.getLocale();
      ResourceBundle bundle = ResourceBundle.getBundle("de.paschty.obsero.messages", locale);
      FXMLLoader loader = new FXMLLoader(getClass().getResource("main-view.fxml"), bundle);
      Parent root = loader.load();
      Scene scene = rootVBox.getScene();
      scene.setRoot(root);
    } catch (IOException e) {
      e.printStackTrace();
    }
  }

  public void stopPolling() {
    if (pollTask != null) pollTask.cancel(false);
    pollTask = null;
    if (scheduler != null) {
      scheduler.shutdownNow();
      scheduler = null;
    }
    lastPollInterval = -1;
  }

  /**
   * Wird aufgerufen, wenn der Benutzer im Datei-Menü auf "Beenden" klickt.
   */
  @FXML
  private void onExitMenuClick() {
    // Hintergrund-Thread beenden
    stopPolling();
    // Fensterposition und -größe speichern
    Stage stage = (Stage) rootVBox.getScene().getWindow();
    AppSettings appSettings = AppSettings.getInstance();
    appSettings.setWindowX(stage.getX());
    appSettings.setWindowY(stage.getY());
    appSettings.setWindowWidth(stage.getWidth());
    appSettings.setWindowHeight(stage.getHeight());
    SettingsManager.save();
    Platform.exit();
  }

}
