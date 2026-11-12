# Install, upgrade and remove OpenUsage

This page covers OU-PLAT-007, OU-DIST-006 and OU-DIST-008. Linux x86_64 is the first platform. Windows and macOS builds are previews, and nobody has run these steps on a real Windows PC or Mac yet.

## Install

### npm

You need Node.js 18 or later.

```sh
npm install -g openusage
openusage doctor
openusage app
```

`openusage doctor` checks the platform, the bundled `ccusage`, the desktop app and its libraries. `openusage app` starts the desktop app. The package runs no install script and downloads nothing at install time or at run time.

On Linux, the desktop app needs WebKitGTK 4.1 and GTK 3. On Ubuntu 24.04:

```sh
sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0
```

The CLI works without these libraries. `openusage app` then prints the install command.

### Debian or Ubuntu package

1. Download `OpenUsage_<version>_amd64.deb` and `SHA256SUMS` from the release.
2. Make sure that the digests match:

   ```sh
   sha256sum --check --ignore-missing SHA256SUMS
   ```

3. Install the package. apt also installs WebKitGTK 4.1 and GTK 3.

   ```sh
   sudo apt install ./OpenUsage_<version>_amd64.deb
   ```

The package installs the desktop app. The `openusage` command comes with the npm package.

### From source

See [Build from source](../CONTRIBUTING.md#build-from-source).

## Upgrade

1. In the app, make a backup: Settings, Encrypted backups, then **Create a backup**.
2. Install the new version over the old one:

   ```sh
   npm install -g openusage@latest
   ```

   For the Debian package, install the new `.deb` with `sudo apt install`.

3. Start the app. It opens your existing workspace and updates its database.

An older version does not open a database that a newer version updated. It shows "This database requires a newer version of OpenUsage." To go back, restore the backup with the older version.

## Uninstall

Uninstalling keeps your data. To delete the data too, do the steps in [Delete your local data](#delete-your-local-data) first.

```sh
npm uninstall -g openusage
```

For the Debian package:

```sh
sudo apt remove open-usage
```

## Remove a profile

On Linux, use these steps for an active profile:

1. Open **Settings → Manage accounts → View usage → Manage profile → Remove**.
2. Choose **Archive History** or **Delete History**.
3. Read the confirmation.
4. Select its approval checkbox.
5. Select the matching history action.

For an archived profile, open **View usage → Delete history**.
Archive History keeps the profile and its saved history. Delete History removes
its owned local records. Tool homes, saved sign-in, and provider subscriptions
remain. [SC-026](srs-clarifications.md#sc-026-profile-removal-and-retained-delivery-controls)
lists retained account records, delivery controls, exports, and backups.

## Delete your local data

### In the app

On Linux and macOS, open Settings, then select **Delete all local data**. This deletes the workspace, its database key and the account sessions that OpenUsage keeps. It keeps the display size, the files that you exported or shared, and your backups.

Windows does not have this action yet.

### By hand

Close the app first. Then delete these folders:

| System | Folders |
|---|---|
| Linux | `~/.local/share/app.openusage.desktop` and `~/.config/app.openusage.desktop`. If you set `XDG_DATA_HOME` or `XDG_CONFIG_HOME`, the folders are in those locations. |
| macOS | `~/Library/Application Support/app.openusage.desktop` |
| Windows | `%LOCALAPPDATA%\app.openusage.desktop` and `%APPDATA%\app.openusage.desktop` |

On Linux, a run on November 8, 2026 confirmed these folders. The macOS and Windows rows come from the code, and no real machine has confirmed them.

Then delete the database key from the OS credential store, if OpenUsage keeps it there. It has the service `app.openusage.desktop` and the account `primary-database-key-v1`. A workspace with a password keeps its key in the workspace folder instead.

| System | Command or place |
|---|---|
| Linux | `secret-tool clear service app.openusage.desktop username primary-database-key-v1` |
| macOS | `security delete-generic-password -s app.openusage.desktop -a primary-database-key-v1` |
| Windows | Credential Manager, then Windows Credentials. Remove the entry for `app.openusage.desktop`. |

These items stay until you delete them:

- Exports and share images in `Downloads/OpenUsage`.
- Backups that you saved.
- Your own Codex installation and its sign-in. OpenUsage keeps its account sessions in its workspace, apart from your Codex home.
- The logs of your AI tools. OpenUsage reads only usage numbers from them and never changes them.

## Unsigned builds

OpenUsage builds have no paid code signing. Release notes say if a build has signing.

| System | What an unsigned build does |
|---|---|
| Linux | Linux does not check signatures for these packages. Check the SHA-256 digests in `SHA256SUMS` before you install. |
| macOS | Gatekeeper can stop the first start of an app that is not notarized. If macOS says that it cannot check the developer, open System Settings, then Privacy & Security, and select Open Anyway. |
| Windows | SmartScreen can show "Windows protected your PC". Check the digest first. Then select More info, then Run anyway. |

The macOS and Windows rows describe the usual behavior of each system. They were not tested with an OpenUsage build.

## What the bundled ccusage can do

See [Bundled ccusage](../SECURITY.md#bundled-ccusage) in the security policy.
