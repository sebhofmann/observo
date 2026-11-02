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
  requires com.google.guice;
  requires jakarta.inject;

  opens de.paschty.observo to javafx.fxml;
  opens de.paschty.observo.monitor.zabbix to com.google.guice;
  opens de.paschty.observo.monitor to com.google.guice;
  opens de.paschty.observo.di to com.google.guice;
  exports de.paschty.observo;
}
