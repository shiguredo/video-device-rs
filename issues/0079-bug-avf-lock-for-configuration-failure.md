# AVF の lockForConfiguration 失敗を無視して要求と違う設定で起動する

- Priority: Medium
- Created: 2026-11-25
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-avf-lock-for-configuration-failure
- Polished: {YYYY-MM-DD}

## 目的

AVFoundation バックエンドで、`AVCaptureDevice lockForConfiguration:` が失敗した
場合に、解像度とフレームレートを設定しないままセッションを起動する状態を直す。
利用者は要求と違う設定のフレームを受け取り続けるが、エラーもログも出ない。

## 優先度根拠

- Medium。他のアプリがカメラをロックしている場合など、通常の利用で起こりうる。
  `VideoCaptureConfig` の要求値と実際の設定が一致しないのに成功扱いになる
- `src/video_avf.m` の `video_avf_session_create` は
  `lockForConfiguration:` の戻り値を条件に設定処理を行い、失敗側の処理が無い

## 現状

`src/video_avf.m` の `video_avf_session_create` は次の構造で設定を行う。

```objc
if ([device lockForConfiguration:&error]) {
    /* activeFormat とフレームレートを設定 */
    [device unlockForConfiguration];
}
```

失敗した場合は `activeFormat` もフレームレートも設定されないまま、
`VideoCapture` の作成は成功する。`config()` は要求値を返すため、利用者からは
要求どおり設定されたように見える。

## 設計方針

- `lockForConfiguration:` が失敗したら `video_avf_session_create` を失敗させる。
  `NSError` の内容を英語のログに 1 回出し、Rust 側で
  `Error::SessionCreateFailed` として扱う
- 設定に失敗した状態でセッションを起動するより、明示的に失敗させる方が
  利用者の誤解を防げる
- ロック失敗と権限拒否を区別する必要がある場合は、エラー写像の方針と合わせる
- `unlockForConfiguration` は成功時だけ呼ぶ現在の構造を維持する

## 完了条件

- 他のプロセスがカメラをロックしている状況で `VideoCapture::new` がエラーを返す
- 失敗時に英語のログが 1 回出て、原因が分かる
- ロックに成功する通常の環境では、従来どおり `activeFormat` と fps が設定される
- macOS 実機で、2 プロセスによるロック競合を使って確認する

## 解決方法

{完了時に記入}
