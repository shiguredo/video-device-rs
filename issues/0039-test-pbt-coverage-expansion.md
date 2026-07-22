# PBT カバレッジの拡充

- Priority: Medium
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/add-pbt-coverage-expansion
- Polished: {YYYY-MM-DD}

## 目的

プロジェクト規約「PBT で実現できるものは PBT で書く」に従い、PBT のカバレッジを拡充する。具体的には (1) `frame_math` 全 7 関数の PBT を新規作成する、(2) `PixelFormat::Unknown(u32)` を PBT の Strategy に追加する。

## 優先度根拠

- Medium。既存コードのバグではないが、プロジェクト規約違反であり、純粋関数の性質検証が不十分
- `frame_math` は整数演算 + オーバーフローチェックの純粋関数であり、PBT の典型対象
- `PixelFormat::Unknown` の `from_raw` / `to_raw` ラウンドトリップが一切検証されていない
- `/review-code` の重要指摘として確認

## 現状

### frame_math に PBT がない

`src/frame_math.rs:1-83` の全 7 関数（`nv12_plane_sizes`, `i420_plane_sizes`, `yuy2_packed_frame_bytes`, `nv12_packed_frame_bytes`, `i420_packed_frame_bytes`, `yuy2_packed_frame_bytes_win`, `mjpeg_payload_bytes`）は純粋な整数演算 + オーバーフローチェックの関数。単体テスト（`src/frame_math.rs:85-191`）はあるが PBT がない。`pbt/tests/prop_frame_math.rs` が存在しない。

### PixelFormat::Unknown が PBT で生成されない

`pbt/tests/prop_types.rs:4-11`:

```rust
fn arbitrary_pixel_format() -> impl Strategy<Value = PixelFormat> {
    prop_oneof![
        Just(PixelFormat::Nv12),
        Just(PixelFormat::Yuy2),
        Just(PixelFormat::I420),
        Just(PixelFormat::Mjpeg),
    ]
}
```

`Unknown(u32)` が含まれないため、`to_raw_matches_expected_fourcc` の `PixelFormat::Unknown(_) => {}` 空アーム（:21）は到達不能。`from_raw(x).to_raw() == x` のラウンドトリップが任意の `u32` で検証されていない。

## 設計方針

### (1) pbt/tests/prop_frame_math.rs の新規作成

各関数に対して以下の性質を検証する:

- 任意の `i32` 入力で panic しない
- `Some` が返るなら入力は全て正の値
- `None` が返るなら入力が非正またはオーバーフロー
- 既知の小さい値で手計算と一致する
- NV12: `Some((y, uv))` なら `y == stride * height` かつ `uv == stride_uv * div_ceil(height, 2)`
- I420: `Some((y, uv))` なら `y == stride * height` かつ `uv == stride_uv * div_ceil(height, 2) * 2`
- YUY2: `Some(bytes)` なら `bytes == stride * height`

`#[cfg]` で条件コンパイルされる関数（`enable_mf` 系、`enable_mjpeg` 系）は、対応する cfg を付けたテストでカバーする。

### (2) arbitrary_pixel_format に Unknown を追加

```rust
fn arbitrary_pixel_format() -> impl Strategy<Value = PixelFormat> {
    prop_oneof![
        Just(PixelFormat::Nv12),
        Just(PixelFormat::Yuy2),
        Just(PixelFormat::I420),
        Just(PixelFormat::Mjpeg),
        any::<u32>().prop_map(PixelFormat::Unknown),
    ]
}
```

`to_raw_matches_expected_fourcc` の `Unknown` アームを `assert_eq!(pf.to_raw(), raw)` に修正する。

`from_raw` / `to_raw` のラウンドトリップテストを追加する:

```rust
proptest! {
    #[test]
    fn from_raw_to_raw_roundtrip(raw in any::<u32>()) {
        // from_raw は enable_avf/v4l2/pipewire 時のみ利用可能
        // pbt は default-features = false のため to_raw のみ検証
    }
}
```

注: `pbt/Cargo.toml` は `default-features = false` のため `from_raw` はコンパイル対象外。`to_raw` のラウンドトリップ（`PixelFormat::Unknown(raw).to_raw() == raw`）のみ検証する。

## 完了条件

- `pbt/tests/prop_frame_math.rs` を新規作成し、frame_math 全関数の PBT を追加する
- `pbt/tests/prop_types.rs` の `arbitrary_pixel_format` に `Unknown(u32)` を追加する
- `cargo test --workspace` が通る
- `CHANGES.md` の `## develop` の `### misc` に `[UPDATE]` エントリを追加する

## 解決方法

{完了時に記入}
