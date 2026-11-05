//! 実機デバイスが必要なテスト向けの共通ヘルパー。

/// 実機デバイスが必要なテストを実行してよいかを返す。
///
/// 実機を掴むテストは CI の Device Test ジョブだけで実行する。
/// そのジョブだけが環境変数 `VIDEO_DEVICE_DEVICE_TEST` を設定し、
/// 未設定の環境 (実機のない通常の CI や開発機) ではテストをスキップする。
///
/// `#[ignore]` は規約で禁止されているため、代わりにこの関数で実行可否を判定する。
pub fn device_test_enabled() -> bool {
    std::env::var_os("VIDEO_DEVICE_DEVICE_TEST").is_some()
}
