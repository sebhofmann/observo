# observo

observo is a JavaFX application for monitoring and displaying Zabbix messages.

## Prerequisites
- Java 17 or higher
- Maven

## Build and Run

1. **Build the project:**

   ```sh
   mvn clean package
   ```

   This produces the application jar, copies all runtime dependencies and creates a platform specific app image under `target/dist/` via `jpackage`.

2. **Run the application:**

   The generated app image is located in `target/dist/`.

   - Linux: `target/dist/Observo/bin/Observo`
   - Windows: `target/dist/Observo/Observo.exe`
   - macOS: open the bundle `target/dist/Observo.app`

## Configuration
- Settings (server, language, window size) are saved automatically when exiting the application.
- The language can be changed in the settings menu.
- Zabbix server configuration is available via the menu.

## Continuous Integration
- Every push to `master` runs the "Build Distributables" GitHub Action and uploads the platform-specific bundles as workflow artifacts (`observo-linux.tar.gz`, `observo-macos.tar.gz`, `observo-windows.zip`).
- Publishing a release in GitHub automatically rebuilds the bundles and attaches the same artifacts to the release.

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
