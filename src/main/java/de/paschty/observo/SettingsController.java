package de.paschty.observo;

import com.google.inject.Inject;
import javafx.fxml.FXML;
import javafx.scene.control.ComboBox;
import javafx.stage.Stage;
import java.util.Locale;

public class SettingsController {

    private final LanguageManager languageManager;
    private final SettingsManager settingsManager;

    @Inject
    public SettingsController(LanguageManager languageManager, SettingsManager settingsManager) {
        this.languageManager = languageManager;
        this.settingsManager = settingsManager;
    }

    @FXML
    private ComboBox<String> languageComboBox;

    @FXML
    public void initialize() {
        languageComboBox.getItems().addAll("Deutsch", "English");
        Locale current = languageManager.getLocale();
        String lang = current.getLanguage();
        if ("de".equals(lang)) {
            languageComboBox.getSelectionModel().select("Deutsch");
        } else if ("en".equals(lang)) {
            languageComboBox.getSelectionModel().select("English");
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
        languageManager.setLocale(selectedLocale);
        settingsManager.save();
        Stage stage = (Stage) languageComboBox.getScene().getWindow();
        stage.close();
    }

    @FXML
    protected void onCancelClick() {
        Stage stage = (Stage) languageComboBox.getScene().getWindow();
        stage.close();
    }
}

