# Dynamic Profiles from Remote URLs

**Issue**: #142
**Date**: 2026-02-14
**Status**: Approved

## Summary

Load profile definitions from remote URLs (e.g., team-shared profiles on a web server or Git repo), merge them into the local profile list, and keep them fresh via configurable auto-refresh.

## Architecture

### New Module: `src/profile/dynamic.rs`

Handles fetching, caching, and merging remote profile sources.

### Core Types

```rust
/// A remote profile source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DynamicProfileSource {
    url: String,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default = "default_refresh_interval")]
    refresh_interval_secs: u64,        // default 1800 (30 min)
    #[serde(default = "default_max_size")]
    max_size_bytes: usize,             // default 1_048_576 (1 MB)
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    conflict_resolution: ConflictResolution,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
enum ConflictResolution {
    #[default]
    LocalWins,
    RemoteWins,
}
```

### Profile Source Tracking

Add a runtime-only field to `Profile`:

```rust
#[derive(Debug, Clone, Default)]
enum ProfileSource {
    #[default]
    Local,
    Dynamic { url: String, last_fetched: Option<SystemTime> },
}
```

This field is `#[serde(skip)]` — not persisted in local `profiles.yaml`, set at runtime during merge.

### Config Additions

New field in `Config`:

```yaml
dynamic_profile_sources:
  - url: "https://example.com/team-profiles.yaml"
    headers:
      Authorization: "Bearer token123"
    refresh_interval_secs: 1800
    max_size_bytes: 1048576
    enabled: true
    conflict_resolution: local_wins
```

## Data Flow

1. **Startup**: Read cached profiles from `~/.config/par-term/cache/dynamic_profiles/<url_sha256>.yaml`
2. **Merge**: Merge cached remote profiles into `ProfileManager`, marking with `source: Dynamic(url)`
3. **Background fetch**: Spawn a tokio task per source that fetches on its configured interval
4. **On success**: Validate YAML, check size limit, write to cache, send update via `mpsc` channel
5. **Main loop**: Check channel in `about_to_wait()`, merge updated profiles into `ProfileManager`
6. **On failure**: Log warning, keep cached version, update status indicator in Settings UI

## Conflict Resolution

When a remote profile has the same **name** as a local profile:

- **LocalWins** (default): Skip the remote profile
- **RemoteWins**: Remote profile replaces the local one in the merged list

Remote profiles never overwrite local profiles by **ID** — only name-based conflict resolution applies. Remote profiles get new UUIDs on each fetch if they don't already have them.

## Caching

- **Cache directory**: `~/.config/par-term/cache/dynamic_profiles/`
- **File per source**: `<sha256_of_url>.yaml` (the fetched profile data)
- **Metadata sidecar**: `<sha256_of_url>.meta` (JSON with last fetch timestamp, ETag, status)
- **Startup behavior**: Load from cache immediately (no network delay), then refresh in background

## Security

- HTTPS enforced by default (warn on HTTP, configurable to allow)
- Configurable max download size per source (default 1 MB)
- YAML validation: reject non-list root elements, reject malformed profiles
- 10-second fetch timeout (uses existing `ureq` dependency)
- Dynamic profiles cannot overwrite local profiles by ID

## Settings UI

New section in the Profiles tab (or a dedicated "Dynamic Sources" sub-section):

- **Source list**: URL, status (OK/Error/Fetching), last fetch time
- **Add/Edit/Remove** sources
- **Per-source controls**: URL input, key-value header editor, interval slider, max size input, enabled toggle, conflict resolution dropdown
- **Refresh Now** button per source + **Refresh All** button
- **Visual indicator** on dynamic profiles in the profile list (cloud icon or "[dynamic]" badge)

### Search Keywords

Add to `tab_search_keywords()`: "dynamic", "remote", "url", "fetch", "refresh", "team", "shared", "download", "sync"

## Keybinding

New action: `reload_dynamic_profiles` — triggers manual refresh of all enabled sources.

## Error Handling

| Failure | Behavior |
|---------|----------|
| Network error | Log warning, keep cached version, show error status in UI |
| Invalid YAML | Log error, skip source, keep previous cache |
| Size exceeded | Log error, skip source, keep previous cache |
| Timeout (10s) | Treat as network error |

## Testing

- Unit tests: YAML parsing/validation, merge logic (both conflict modes), cache read/write
- Unit tests: `DynamicProfileSource` serialization roundtrip
- Integration test: fetch from file:// URL or mock server
- Settings UI test: source management CRUD
