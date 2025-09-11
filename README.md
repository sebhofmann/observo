# observo

observo is a JavaFX application for monitoring and displaying Zabbix messages.

## Prerequisites
- Java 17 or higher
- Maven

## Build and Run

1. **Build the project:**

   ```sh
   mvn clean install javafx:jlink
   ```

   This will build the project and create a runnable runtime image in the `target/` directory.

2. **Run the application:**

   The generated image is located in `target/observo/`. You can start the application with:

   ```sh
   target/observo/bin/observo
   ```

## Configuration
- Settings (server, language, window size) are saved automatically when exiting the application.
- The language can be changed in the settings menu.
- Zabbix server configuration is available via the menu.

## Features
- Polling Zabbix problems
- Message filtering
- Acknowledge function with comment
- Multilingual (German/English)
- Notifications and sounds

## Notes
- When closing the window, the application is minimized and continues running in the background. To fully exit, use the menu "File > Exit".

## License
This project is licensed under the GPL-3.0 License.
