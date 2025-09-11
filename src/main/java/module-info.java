module de.paschty.observo {
  requires javafx.controls;
  requires javafx.fxml;

  requires org.controlsfx.controls;
  requires java.net.http;
  requires org.json;
  requires org.apache.logging.log4j;
  requires org.apache.logging.log4j.core;
  requires javafx.media;
  requires java.desktop;

  opens de.paschty.observo to javafx.fxml;
  exports de.paschty.observo;
}