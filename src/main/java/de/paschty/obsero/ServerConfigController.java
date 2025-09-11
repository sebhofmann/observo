package de.paschty.obsero;

import de.paschty.obsero.monitor.Configuration;
import de.paschty.obsero.monitor.ConfigurationValue;
import de.paschty.obsero.monitor.TextField;
import de.paschty.obsero.monitor.PasswordField;
import de.paschty.obsero.monitor.NumberField;
import de.paschty.obsero.monitor.BooleanField;
import javafx.fxml.FXML;
import javafx.scene.control.*;
import javafx.scene.layout.HBox;
import javafx.scene.layout.VBox;
import javafx.stage.Stage;
import javafx.fxml.Initializable;
import javafx.collections.FXCollections;
import javafx.scene.control.ComboBox;
import de.paschty.obsero.monitor.zabbix.ZabbixServerConfiguration;
import java.net.URL;
import java.util.ResourceBundle;
import java.util.HashMap;
import java.util.Map;

public class ServerConfigController implements Initializable {

  private final Map<ConfigurationValue<?>, Control> valueControls = new HashMap<>();
  @FXML
  private VBox formContainer;
  @FXML
  private ComboBox<String> serverTypeComboBox;
  private Configuration configuration;
  private ResourceBundle resources;

  @Override
  public void initialize(URL location, ResourceBundle resources) {
    this.resources = resources;
    serverTypeComboBox.setItems(FXCollections.observableArrayList("Zabbix"));
    serverTypeComboBox.getSelectionModel().selectFirst();
    serverTypeComboBox.setOnAction(e -> onServerTypeChanged());
    // Initiales Formular anzeigen
    setConfiguration(new ZabbixServerConfiguration());
  }

  private void onServerTypeChanged() {
    String selected = serverTypeComboBox.getSelectionModel().getSelectedItem();
    if ("Zabbix".equals(selected)) {
      setConfiguration(new ZabbixServerConfiguration());
    }
    // Hier können weitere Typen ergänzt werden
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
      // Hilfe-Button falls HelpKey vorhanden
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
    for (Map.Entry<ConfigurationValue<?>, Control> entry : valueControls.entrySet()) {
      ConfigurationValue<?> value = entry.getKey();
      Control control = entry.getValue();
      if (value instanceof TextField textField) {
        textField.setValue(
            ((javafx.scene.control.TextField) control).getText());
      } else if (value instanceof PasswordField passwordField) {
        passwordField.setValue(
            ((javafx.scene.control.PasswordField) control).getText());
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
    // Globale Einstellungen setzen
    AppSettings.getInstance().setServerConfiguration(configuration);
    // Einstellungen persistent speichern
    SettingsManager.save();
    Stage stage = (Stage) formContainer.getScene().getWindow();
    stage.close();
  }

  @FXML
  protected void onCancelClick() {
    Stage stage = (Stage) formContainer.getScene().getWindow();
    stage.close();
  }
}
