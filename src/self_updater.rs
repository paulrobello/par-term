//! Self-update functionality re-exports from `par-term-update`.
pub use par_term_update::self_updater::{
    DownloadUrls, InstallationType, UpdateResult, cleanup_old_binary, compute_data_hash,
    detect_installation, get_asset_name, get_binary_download_url, get_checksum_asset_name,
    get_download_urls, perform_update,
};
