package de.paschty.observo;

import com.google.inject.Inject;
import de.paschty.observo.monitor.Configuration;
import de.paschty.observo.monitor.ConfigurationValue;
import de.paschty.observo.monitor.ServerManager;
import de.paschty.observo.monitor.ServerProvider;
import de.paschty.observo.monitor.TextField;
import de.paschty.observo.monitor.PasswordField;
import de.paschty.observo.monitor.NumberField;
import de.paschty.observo.monitor.BooleanField;
import java.net.URL;
import java.util.HashMap;
import java.util.Map;
import java.util.ResourceBundle;
import javafx.collections.FXCollections;
import javafx.fxml.FXML;
import javafx.fxml.Initializable;
import javafx.scene.control.Alert;
import javafx.scene.control.Button;
import javafx.scene.control.CheckBox;
import javafx.scene.control.ComboBox;
import javafx.scene.control.Control;
import javafx.scene.control.Label;
import javafx.scene.control.ListCell;
import javafx.scene.layout.HBox;
import javafx.scene.layout.VBox;
import javafx.stage.Stage;

public class ServerConfigController implements Initializable {

  private final SettingsManager settingsManager;
  private final ServerManager serverManager;
  private final Map<ConfigurationValue<?>, Control> valueControls = new HashMap<>();
  @FXML
  private VBox formContainer;
  @FXML
  private ComboBox<ServerProvider> serverTypeComboBox;
  private Configuration configuration;
  private ResourceBundle resources;

  @Inject
  public ServerConfigController(SettingsManager settingsManager,
                                ServerManager serverManager) {
    this.settingsManager = settingsManager;
    this.serverManager = serverManager;
  }

  @Override
  public void initialize(URL location, ResourceBundle resources) {
    this.resources = resources;
    var providers = FXCollections.observableArrayList(serverManager.getProviders());
    serverTypeComboBox.setItems(providers);
    serverTypeComboBox.setCellFactory(cb -> new ListCell<>() {
      @Override
      protected void updateItem(ServerProvider item, boolean empty) {
        super.updateItem(item, empty);
        setText(empty || item == null ? null : resolveDisplayName(item));
      }
    });
    serverTypeComboBox.setButtonCell(new ListCell<>() {
      @Override
      protected void updateItem(ServerProvider item, boolean empty) {
        super.updateItem(item, empty);
        setText(empty || item == null ? null : resolveDisplayName(item));
      }
    });
    serverTypeComboBox.setOnAction(e -> onServerTypeChanged());
    ServerProvider activeProvider = serverManager.getActiveProvider();
    if (activeProvider != null) {
      serverTypeComboBox.getSelectionModel().select(activeProvider);
      setConfiguration(serverManager.getConfiguration(activeProvider.id()));
    } else if (!providers.isEmpty()) {
      serverTypeComboBox.getSelectionModel().selectFirst();
      onServerTypeChanged();
    }
  }

  private void onServerTypeChanged() {
    ServerProvider selected = serverTypeComboBox.getSelectionModel().getSelectedItem();
    if (selected != null) {
      setConfiguration(serverManager.getConfiguration(selected.id()));
    }
  }

  private String resolveDisplayName(ServerProvider provider) {
    if (provider == null) {
      return null;
    }
    String key = provider.displayI18nKey();
    if (key != null && resources != null && resources.containsKey(key)) {
      return resources.getString(key);
    }
    if (key != null && !key.isBlank()) {
      return key;
    }
    return provider.id();
  }

  public void setConfiguration(Configuration configuration) {
    this.configuration = configuration;
    formContainer.getChildren().clear();
    valueControls.clear();
    for (ConfigurationValue<?> value : configuration.getValues()) {
      String labelKey = "serverConfig.label." + value.getKey();
      String labelText;
      if (resources != null && resources.containsKey(labelKey)) {
        labelText = resources.getString(labelKey);
      } else {
        labelText = value.getKey();
      }
      Label label = new Label(labelText);
      Control control;
      switch (value) {
        case TextField textValue ->
            control = new javafx.scene.control.TextField(textValue.getValue());
        case PasswordField passValue -> {
          javafx.scene.control.PasswordField controlField = new javafx.scene.control.PasswordField();
          controlField.setText(passValue.getValue());
          control = controlField;
        }
        case NumberField numValue ->
            control = new javafx.scene.control.TextField(numValue.getValue().toString());
        case BooleanField boolValue -> {
          CheckBox checkBox = new CheckBox();
          checkBox.setSelected(boolValue.getValue());
          control = checkBox;
        }
        default -> {
          continue;
        }
      }
      if (value.getHelpKey() != null) {
        Button helpButton = new Button("?");
        helpButton.setFocusTraversable(false);
        helpButton.setStyle("-fx-font-size: 12px; -fx-padding: 2 6;");
        helpButton.setOnAction(e -> {
          String helpText = resources != null && resources.containsKey(value.getHelpKey())
              ? resources.getString(value.getHelpKey())
              : value.getHelpKey();
          Alert alert = new Alert(Alert.AlertType.INFORMATION);
          alert.setTitle(labelText + " - Hilfe");
          alert.setHeaderText(labelText);
          alert.setContentText(helpText);
          alert.showAndWait();
        });
        HBox hBox = new HBox(6, control, helpButton);
        VBox vBox = new VBox(label, hBox);
        formContainer.getChildren().add(vBox);
      } else {
        VBox vBox = new VBox(label, control);
        formContainer.getChildren().add(vBox);
      }
      valueControls.put(value, control);
    }
  }

  @FXML
  protected void onSaveConfigClick() {
    ServerProvider selectedProvider = serverTypeComboBox.getSelectionModel().getSelectedItem();
    if (selectedProvider == null) {
      return;
    }
    for (Map.Entry<ConfigurationValue<?>, Control> entry : valueControls.entrySet()) {
      ConfigurationValue<?> value = entry.getKey();
      Control control = entry.getValue();
      if (value instanceof TextField textField) {
        textField.setValue(((javafx.scene.control.TextField) control).getText());
      } else if (value instanceof PasswordField passwordField) {
        passwordField.setValue(((javafx.scene.control.PasswordField) control).getText());
      } else if (value instanceof NumberField numberField) {
        String text = ((javafx.scene.control.TextField) control).getText();
        try {
          numberField.setValue(Integer.parseInt(text));
        } catch (NumberFormatException ignored) {
        }
      } else if (value instanceof BooleanField booleanField) {
        booleanField.setValue(((CheckBox) control).isSelected());
      }
    }
    serverManager.getConfiguration(selectedProvider.id());
    serverManager.activate(selectedProvider.id());
    serverManager.updateActiveConfiguration(configuration);
    settingsManager.save();
    Stage stage = (Stage) formContainer.getScene().getWindow();
    stage.close();
  }

  @FXML
  protected void onCancelClick() {
    Stage stage = (Stage) formContainer.getScene().getWindow();
    stage.close();
  }
}
