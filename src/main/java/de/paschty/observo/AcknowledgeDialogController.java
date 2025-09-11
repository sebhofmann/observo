package de.paschty.observo;

import javafx.fxml.FXML;
import javafx.scene.control.TextArea;
import javafx.scene.control.DialogPane;

public class AcknowledgeDialogController {
    @FXML
    private TextArea messageTextArea;
    @FXML
    private DialogPane dialogPane;

    public String getMessage() {
        return messageTextArea.getText();
    }
}

