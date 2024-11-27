/// This build script ensures that only one of the `local_with_classification`,
/// `local_with_db_only`, or `default` features is enabled at a time.
fn main() {
    let local_with_classification =
        std::env::var("CARGO_FEATURE_LOCAL_WITH_CLASSIFICATION").is_ok();
    let local_with_db_only = std::env::var("CARGO_FEATURE_LOCAL_WITH_DB_ONLY").is_ok();
    let default = std::env::var("CARGO_FEATURE_DEFAULT").is_ok();

    let enabled_features = [local_with_classification, local_with_db_only, default];

    if enabled_features.iter().filter(|&&x| x).count() > 1 {
        panic!("Only one of 'local_with_classification', 'local_with_db_only', or 'default' features can be enabled at a time.");
    }
}
