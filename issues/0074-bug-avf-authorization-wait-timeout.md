# AVF の権限要求が無期限にブロックする

- Priority: High
- Created: 2026-11-25
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-avf-authorization-wait-timeout
- Polished: {YYYY-MM-DD}

## 目的

macOS の AVFoundation バックエンドで、カメラ権限が未決定のときに
`VideoCapture::new` が永久に戻らない経路をなくす。TCC のダイアログを出せない
環境（SSH、launchd、画面ロック中、Info.plist の `NSCameraUsageDescription`
不備など）では、利用側のスレッドがハングしたまま復帰できない。

## 優先度根拠

- High。ハングはエラーと違って利用者が検知・回復できず、プロセス全体を
  止めうる。`VideoCapture::new` は公開 API の入口であり、通常の利用から到達する
- `/review-code` 相当の監査で、権限待ちにタイムアウトが無いことを確認した

## 現状

`src/video_avf.m` の `video_avf_session_create` は、権限が
`AVAuthorizationStatusNotDetermined` のとき `requestAccessForMediaType` を呼び、
`dispatch_semaphore_wait` で完了を待つ。待ち時間は `DISPATCH_TIME_FOREVER` で、
タイムアウトもログも無い。

```c
dispatch_semaphore_wait(sema, DISPATCH_TIME_FOREVER);
```

完了ハンドラが呼ばれない環境では、この待機がそのまま残る。
Rust 側の `FfiCaptureImpl::new_avf` はこの C 関数から戻る前提で書かれており、
`VideoCapture::new` も例外ではない。

## 設計方針

- 待機に有限のタイムアウトを設ける。値は定数として 1 箇所に置き、
  そのコメントで根拠を説明する
- `dispatch_semaphore_wait` の戻り値が `0` 以外なら、権限が得られなかったものとして
  `NULL` を返す。TCC の応答が来ない場合と明示的に拒否された場合を区別できるなら
  区別する
- Rust 側のエラー写像は、権限拒否を `Error::DeviceAccessDenied` として扱う方針に
  合わせる。既存の `SessionCreateFailed` に丸める場合は、その理由を doc に書く
- 公開 doc に、権限が未決定のときは OS のダイアログが出るまで `new` がブロックする
  ことと、その最大待ち時間を書く

## 完了条件

- TCC の応答が返らない状況でも `VideoCapture::new` が有限時間でエラーを返す
- 権限拒否時に `Error::DeviceAccessDenied` 相当のエラーになり、成功扱いにならない
- `VideoCapture::new` の doc にブロッキングとタイムアウトが明記されている
- ヘッドレス環境またはダイアログを出せない環境でハングしないことを確認する

## 解決方法

{完了時に記入}
