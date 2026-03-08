# Import/Export Preferences

par-term supports importing and exporting terminal configuration for backup, sharing, and team standardization.

## Table of Contents
- [Overview](#overview)
- [Exporting Preferences](#exporting-preferences)
- [Importing Preferences](#importing-preferences)
  - [Import from File](#import-from-file)
  - [Import from URL](#import-from-url)
  - [Import Modes](#import-modes)
- [Settings UI](#settings-ui)
- [Related Documentation](#related-documentation)

## Overview

The import/export system reads and writes par-term configuration in YAML format, allowing you to back up settings, share configurations between machines, or distribute team-standard configurations.

```mermaid
graph TD
    Config[Current Config]
    Export[Export to YAML]
    ImportFile[Import from File]
    ImportURL[Import from URL]
    Validate[Validate Config]
    Apply[Apply Settings]

    Config -->|Export| Export
    ImportFile --> Validate
    ImportURL --> Validate
    Validate -->|Valid| Apply
    Apply -->|Replace or Merge| Config

    style Config fill:#e65100,stroke:#ff9800,stroke-width:3px,color:#ffffff
    style Export fill:#1b5e20,stroke:#4caf50,stroke-width:2px,color:#ffffff
    style ImportFile fill:#0d47a1,stroke:#2196f3,stroke-width:2px,color:#ffffff
    style ImportURL fill:#4a148c,stroke:#9c27b0,stroke-width:2px,color:#ffffff
    style Validate fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
    style Apply fill:#880e4f,stroke:#c2185b,stroke-width:2px,color:#ffffff
```

## Exporting Preferences

Export the current configuration to a YAML file:

1. Open Settings (`F12` or `Cmd/Ctrl + ,`)
2. Navigate to **Advanced** > **Import/Export Preferences**
3. Click **Export Preferences to File**
4. Choose a location in the native file dialog
5. The current configuration saves as a `.yaml` file

The exported file contains all configuration values.

## Importing Preferences

### Import from File

1. Open Settings > **Advanced** > **Import/Export Preferences**
2. Click **Import & Replace** to completely replace your config, or **Import & Merge** to preserve existing customizations
3. Select a `.yaml` configuration file in the native file dialog
4. The configuration applies immediately

### Import from URL

1. Open Settings > **Advanced** > **Import/Export Preferences**
2. Enter the URL of a configuration file (must start with `http://` or `https://`)
3. Click **Fetch & Replace** to completely replace your config, or **Fetch & Merge** to preserve existing customizations
4. The configuration downloads and applies

### Import Modes

| Mode | Button | Behavior |
|------|--------|----------|
| **Replace** | Import & Replace / Fetch & Replace | Completely replaces the current configuration with the imported values |
| **Merge** | Import & Merge / Fetch & Merge | Only overrides values that differ from defaults, preserving your customizations |

**Merge mode** is recommended when importing partial configurations or when you want to preserve your existing settings while adding specific overrides from the imported file.

**Validation**: All imported configurations are validated before applying. Malformed or invalid YAML files are rejected with an error message displayed in the settings UI.

## Settings UI

All import/export controls are located in **Settings > Advanced > Import/Export Preferences**.

## Related Documentation

- [Window Management](WINDOW_MANAGEMENT.md) - Window and display configuration
- [Profiles](PROFILES.md) - Profile management (separate from main config)
- [Arrangements](ARRANGEMENTS.md) - Window arrangements (separate from main config)
