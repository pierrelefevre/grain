pub(crate) fn get_build_info() -> String {
    let raw_ver = match option_env!("BUILD_VERSION") {
        Some(v) => v,
        None => return "test".to_string(),
    };

    let long_sha = match raw_ver.split('-').next_back() {
        Some(sha) => sha,
        None => return raw_ver.to_string(),
    };

    let short_sha = long_sha.chars().take(7).collect::<String>();

    raw_ver.replace(long_sha, &short_sha)
}
