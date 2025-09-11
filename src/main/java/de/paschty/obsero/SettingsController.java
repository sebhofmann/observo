package de.paschty.obsero;

import javafx.fxml.FXML;
import javafx.scene.control.ComboBox;
import javafx.stage.Stage;
import java.util.Locale;
import de.paschty.obsero.LanguageManager;

public class SettingsController {
    @FXML
    private ComboBox<String> languageComboBox;

    @FXML
    public void initialize() {
        languageComboBox.getItems().addAll("Deutsch", "English");
        Locale current = LanguageManager.getLocale();
        if (current.getLanguage().equals("de")) {
            languageComboBox.getSelectionModel().select("Deutsch");
        } else {
            languageComboBox.getSelectionModel().select("English");
        }
    }

    @FXML
    protected void onSaveClick() {
        String selected = languageComboBox.getSelectionModel().getSelectedItem();
        Locale selectedLocale;
        if ("Deutsch".equals(selected)) {
            selectedLocale = Locale.GERMAN;
        } else {
            selectedLocale = Locale.ENGLISH;
        }
        // Globale Einstellungen setzen
        AppSettings.getInstance().setLocale(selectedLocale);
        LanguageManager.setLocale(selectedLocale);
        // Einstellungen persistent speichern
        SettingsManager.save();
        Stage stage = (Stage) languageComboBox.getScene().getWindow();
        stage.close();
    }

    @FXML
    protected void onCancelClick() {
        Stage stage = (Stage) languageComboBox.getScene().getWindow();
        stage.close();
    }
}
