package de.paschty.observo;

import static javafx.scene.control.ButtonBar.ButtonData.OK_DONE;

import com.google.inject.Inject;
import de.paschty.observo.monitor.Message;
import de.paschty.observo.monitor.Server;
import de.paschty.observo.monitor.zabbix.ZabbixServerConfiguration;

import javafx.animation.PauseTransition;
import javafx.collections.FXCollections;
import javafx.collections.ObservableList;
import javafx.fxml.FXML;
import javafx.fxml.FXMLLoader;
import javafx.scene.Parent;
import javafx.scene.Scene;
import javafx.scene.control.DialogPane;
import javafx.scene.control.Label;
import javafx.scene.control.TableColumn;
import javafx.scene.control.TableRow;
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
import javafx.scene.control.ContextMenu;
import javafx.scene.control.MenuItem;
import javafx.scene.control.Dialog;
import javafx.scene.control.ButtonType;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;

public class MainController {

  private final AppSettings appSettings;
  private final SettingsManager settingsManager;
  private final LanguageManager languageManager;
  private final FXMLLoaderFactory fxmlLoaderFactory;
  private final Server server;

  @Inject
  public MainController(AppSettings appSettings,
                        SettingsManager settingsManager,
                        LanguageManager languageManager,
                        FXMLLoaderFactory fxmlLoaderFactory,
                        Server server) {
    this.appSettings = appSettings;
    this.settingsManager = settingsManager;
    this.languageManager = languageManager;
    this.fxmlLoaderFactory = fxmlLoaderFactory;
    this.server = server;
    var configuration = appSettings.getServerConfiguration();
    if (configuration != null) {
      this.server.setConfiguration(configuration);
    }
  }

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

  @FXML
  private MenuItem acknowledgeMenuItem;

  private ScheduledExecutorService scheduler;
  private ScheduledFuture<?> pollTask;
  private int lastPollInterval = -1;
  private boolean hadMessages = false;
  private boolean hadCriticalMessages = false;

  private static final Logger logger = LogManager.getLogger(MainController.class);

  @FXML
  protected void onHelloButtonClick() {
    welcomeText.setText("Welcome to JavaFX Application!");
  }

  @FXML
  protected void onServerConfigMenuClick() {
    try {
      // ResourceBundle für die aktuelle Sprache laden
      java.util.Locale locale = languageManager.getLocale();
      ResourceBundle bundle = ResourceBundle.getBundle("de.paschty.observo.messages", locale);
      FXMLLoader loader = fxmlLoaderFactory.create(getClass().getResource("server-config-view.fxml"), bundle);
      Parent root = loader.load();
      ServerConfigController controller = loader.getController();
      controller.setConfiguration(appSettings.getServerConfiguration());
      Stage stage = new Stage();
      stage.setTitle(bundle.getString("serverConfig.title"));
      stage.setScene(new Scene(root));
      stage.showAndWait(); // Warten bis geschlossen
      // Nach Schließen: Timer neu starten, falls Intervall geändert
      restartPollingIfNeeded();
      // Server-Konfiguration nach Dialog aktualisieren
      if (server != null) {
        server.setConfiguration(appSettings.getServerConfiguration());
      }
    } catch (IOException e) {
      e.printStackTrace();
    }
  }

  @FXML
  protected void onSettingsMenuClick() {
    try {
      java.util.Locale locale = languageManager.getLocale();
      ResourceBundle bundle = ResourceBundle.getBundle("de.paschty.observo.messages", locale);
      FXMLLoader loader = fxmlLoaderFactory.create(getClass().getResource("settings-view.fxml"), bundle);
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

    // RowFactory für farbige Zeilen und Kontextmenü pro Zeile
    messagesTable.setRowFactory(tableView -> {
      TableRow<Message> row = new TableRow<>() {
        @Override
        protected void updateItem(Message msg, boolean empty) {
          super.updateItem(msg, empty);
          if (msg == null || empty) {
            setStyle("");
            setContextMenu(null); // Kontextmenü entfernen, wenn leer
          } else if (isSelected()) {
            setStyle("-fx-background-color: #1976d2; -fx-text-fill: white;");
          } else {
            switch (msg.getClassification()) {
              case CRITICAL -> setStyle("-fx-background-color: #ffcccc; -fx-text-fill: black;");
              case WARNING -> setStyle("-fx-background-color: #fff8dc; -fx-text-fill: black;");
              case INFO -> setStyle("-fx-background-color: #e6f0ff; -fx-text-fill: black;");
              case RECOVERY -> setStyle("-fx-background-color: #e6ffe6; -fx-text-fill: black;");
              case ACKNOWLEDGED -> setStyle("-fx-background-color: #eeeeee; -fx-text-fill: black;");
              case UNKNOWN -> setStyle("");
            }
          }
          // Kontextmenü nur für nicht-ACKNOWLEDGED Nachrichten
          if (msg != null && !empty && msg.getClassification() != de.paschty.observo.monitor.Classification.ACKNOWLEDGED) {
            ContextMenu contextMenu = new ContextMenu();
            MenuItem acknowledgeItem = new MenuItem("Acknowledge");
            acknowledgeItem.setOnAction(e -> {
              messagesTable.getSelectionModel().select(getIndex());
              onAcknowledgeMenuClick();
            });
            contextMenu.getItems().add(acknowledgeItem);
            setContextMenu(contextMenu);
          } else {
            setContextMenu(null);
          }
        }
      };
      return row;
    });
    //loadMessages();
    startPolling();
  }

  private void startPolling() {
    stopPolling(); // Vorher alles stoppen
    scheduler = Executors.newSingleThreadScheduledExecutor(r -> {
      Thread t = new Thread(r);
      t.setDaemon(true);
      return t;
    });
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
    var config = appSettings.getServerConfiguration();
    if (config instanceof ZabbixServerConfiguration zabbixConfig) {
      return zabbixConfig.getPollIntervalMillis().getValue();
    }
    return 10000;
  }

  private void loadMessages() {
    // Server verwenden
    List<Message> messages = server.pollMessages();
    boolean hasMessages = messages != null && !messages.isEmpty();
    boolean hasCriticalMessages = messages.stream().anyMatch(msg ->
        msg.getClassification() == de.paschty.observo.monitor.Classification.CRITICAL
        || msg.getClassification() == de.paschty.observo.monitor.Classification.WARNING);
    // Notification/Sound nur bei Wechsel von keine zu mindestens eine kritische Nachricht
    if (!hadCriticalMessages && hasCriticalMessages) {
        ResourceBundle bundle = ResourceBundle.getBundle("de.paschty.observo.messages", languageManager.getLocale());
        String newMessageTitle = bundle.getString("notification.newMessages.title");
        String newMessageText = bundle.getString("notification.newMessages.text");
        Platform.runLater(() -> {
            sendSystemNotification(newMessageTitle, newMessageText);
            playNotificationSound();
        });
    }
    hadMessages = hasMessages;
    hadCriticalMessages = hasCriticalMessages;
    Platform.runLater(() -> {
        ResourceBundle bundle = ResourceBundle.getBundle("de.paschty.observo.messages", languageManager.getLocale());
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
      java.util.Locale locale = languageManager.getLocale();
      ResourceBundle bundle = ResourceBundle.getBundle("de.paschty.observo.messages", locale);
      FXMLLoader loader = fxmlLoaderFactory.create(getClass().getResource("main-view.fxml"), bundle);
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
    appSettings.setWindowX(stage.getX());
    appSettings.setWindowY(stage.getY());
    appSettings.setWindowWidth(stage.getWidth());
    appSettings.setWindowHeight(stage.getHeight());
    settingsManager.save();
    Platform.exit();
  }

  @FXML
  private void onAcknowledgeMenuClick() {
    logger.info("Acknowledge menu click triggered");
    ResourceBundle bundle = ResourceBundle.getBundle("de.paschty.observo.messages", languageManager.getLocale());
    Message selectedMessage = messagesTable.getSelectionModel().getSelectedItem();
    if (selectedMessage == null) {
      logger.warn("No message selected");
      Notifications.create()
        .title(bundle.getString("acknowledge.noMessageSelected.title"))
        .text(bundle.getString("acknowledge.noMessageSelected.text"))
        .showWarning();
      return;
    }
    try {
      FXMLLoader loader = fxmlLoaderFactory.create(getClass().getResource(
          "/de/paschty/observo/acknowledge-dialog.fxml"), bundle);
      DialogPane dialogPane = loader.load();
      AcknowledgeDialogController dialogController = loader.getController();
      Dialog<ButtonType> dialog = new Dialog<>();
      dialog.setDialogPane(dialogPane);
      dialog.setTitle(bundle.getString("acknowledge.dialog.title"));
      dialog.initOwner(rootVBox.getScene().getWindow());
      ButtonType result = dialog.showAndWait().orElse(ButtonType.CANCEL);
      logger.info("Result: {}", result);
      if (result.getButtonData().equals(OK_DONE)) {
        String ackMessage = dialogController.getMessage();
        logger.info("SelectedMessage ID: {}, Text: {}", selectedMessage.getId(), ackMessage);
        logger.info("Server: {}", server);
        boolean success = false;
        if (server != null) {
          success = server.acknowledgeMessage(selectedMessage.getId(), ackMessage);
        } else {
          logger.error("Server is null!");
        }
        if (success) {
          Notifications.create()
            .title(bundle.getString("acknowledge.success.title"))
            .text(bundle.getString("acknowledge.success.text"))
            .showInformation();
          PauseTransition pause = new PauseTransition(javafx.util.Duration.seconds(1));
          pause.setOnFinished(ev -> loadMessages());
          pause.play();
        } else {
          Notifications.create()
            .title(bundle.getString("acknowledge.error.title"))
            .text(bundle.getString("acknowledge.error.text"))
            .showError();
        }
      }
    } catch (IOException e) {
      logger.error("Error opening dialog", e);
      Notifications.create()
        .title(bundle.getString("acknowledge.dialogOpenError.title"))
        .text(bundle.getString("acknowledge.dialogOpenError.text"))
        .showError();
    } catch (Exception e) {
      logger.error("Error in onAcknowledgeMenuClick", e);
    }
  }

}
