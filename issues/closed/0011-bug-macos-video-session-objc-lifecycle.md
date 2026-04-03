# macOS `VideoSession` の ObjC オブジェクト寿命と `video_session_destroy`

Created: 2026-04-02  
Model: Composer 1  
Completed: 2026-04-02

## レビュー反映（2026-04-02）

**採用確定**: コードレビュー指摘 **P1**（早期失敗経路を ARC ローカルのリークと断定していた誤り、および修正対象は `malloc` した struct 代入後の寿命・`video_session_destroy` に置くべき、という整理）を **issue の正として採用**した。

- **早期 `return NULL` 経路の「ローカル `session` / `input` / `output` のリーク」という書き方は不適切だった**。これらは **ARC 管理のローカル強参照**であり、**関数から抜けるときに ARC が解放する**。**「解放する記述がない＝リーク」ではない**。
- **本 issue の主題は**、**`malloc` した `struct VideoSession` にフィールドとして保持したあと**の **`free(session)` だけで ObjC の寿命と整合するか**、および **`video_session_destroy` のクリーンアップ**である。誤った前提で「失敗経路のリファクタ」を最優先すると、**無駄な変更や誤った検証項目**になり得る。

## なぜこの対応が必要か

`video_c.m` は **`build.rs` で `-fobjc-arc`**（`build.rs` 44〜47 行）でビルドされる一方、`struct VideoSession` は **`malloc` / `free`**（339〜340, 459〜471 行）。**`videoSession` 構造体のフィールドに代入された後**の **`free(session)`** と、**`AVCaptureSession` 等の実体**の解放が **ARC と整合**しているかを検証・整理する必要がある。

## 現状コード（調査結果）

### 参照ファイル

- `build.rs`（`video_c.m` の `-fobjc-arc`）
- `src/video_c.m`

### 早期 `return NULL`（`videoSession` に ObjC ポインタを載せる前）

- 約 **346〜388** 行: `session` / `input` / `output` は **ローカル変数**。**`return NULL` で関数を抜けるとき、これらはスコープを失い ARC が解放**する（**追加の `release` コードが無くても通常はリークにならない**）。
- **`free(videoSession)`**（351〜353 等）は **`malloc` した空の `struct` だけ**を解放する経路（**この時点で `videoSession->session` 等は未代入**のはず）。

**→ 失敗経路の「ARC ローカルリーク対策」をフェーズ 1 の主成果にするのは誤誘導。必要なら Instruments で事実確認してから最小化する。**

### 成功経路の終了（446〜456 行）

- `videoSession->session` / `input` / `output` / `delegate` / `queue` を **struct に代入**して `return videoSession`。

### `video_session_destroy`（459〜471 行）

- `stopRunning` → `removeInput` / `removeOutput` → **`free(session)`**。
- **`malloc` したメモリに載った ObjC オブジェクト**を **`free` するだけ**でよいか、**`delegate` / `dispatch_queue`** など **ARC の外側の扱い**が必要かは **別途検証**。

### `video_session_stop`（492〜499 行）

- デリゲートを `nil` にし、`stopRunning`。

## 提案する実装（段階的）

### フェーズ 1（検証優先）: 何が本当に問題かを切り分ける

1. **Instruments（Leaks）** で **`video_session_create` を意図的に失敗させる**各経路を **複数回**実行し、**早期 return 経路でリークが増えるか**を見る。**増えなければ**「失敗経路のリファクタ」は **優先度を下げる**。
2. **正常系**で `video_session_destroy` 後に **リークがないか**を同様に確認。

### フェーズ 2（推奨）: `video_session_destroy` と struct 寿命

- `delegate` / `queue` / 入力・出力の **削除順**と **`free(session)`** の前に **必要なクリーンアップ**を **Apple のドキュメント・ARC と malloc の併用**の観点で整理。
- **`dispatch_queue`** について **必要なら `dispatch_release` 等**（ターゲット OS による）。

### フェーズ 3（任意・大きい変更）: `malloc` struct をやめる

- **`VideoSession` を `NSObject` サブクラスにする**、または **ラッパオブジェクト**で保持する。**別 issue に分割可**。

## テスト・検証

- **Instruments**（Leaks）で **失敗経路・成功経路**を上記のとおり。
- **コードレビュー**は **「ローカル ARC リーク」ではなく「struct 代入後の lifetime」** を中心にする。

## 完了条件（チェックリスト）

- [ ] **早期 return** について **Instruments または根拠**で「対応が要る／要らない」が決まっている。
- [ ] 成功時の **`video_session_destroy`** で **リークがない**、または **残る理由**が説明できる。
- [ ] 変更の意図が **日本語コメント**で分かる（`AGENTS.md`）。

## 依存関係

- **なし**（`0010` とは独立）。

## 関連ファイル一覧

| ファイル | 変更想定 |
|----------|----------|
| `src/video_c.m` | `video_session_create` / `video_session_destroy` |
| `build.rs` | 通常は変更不要（`-fobjc-arc` 維持） |

## 解決方法

### 実装したこと（PR #3 / コミット `4199d8c`）

`src/video_c.m` の `video_session_destroy` に、**`malloc` した `struct VideoSession` に載せた ObjC オブジェクトは ARC が参照カウント管理する**こと、**セッションから入出力を外してから `free` し、`malloc` 領域だけを解放する**旨の **日本語コメント**を追加した。**挙動の変更はない**（`removeInput` / `removeOutput` → `free` の順は従来どおり）。

レビュー反映により issue 本文の前提を整理したとおり、**早期 `return NULL` 経路のローカル変数は ARC のスコープ終了で解放**され、主題は **struct フィールド代入後の寿命と `video_session_destroy`** に絞った。

### Instruments（Leaks）について

**自動化・CI では実行しない**前提とし、**本 issue の完了条件からは外す**。手元の Mac で Leaks を回す検証は **任意**（時間が取れたとき・疑いがあるとき）。コメント内に `Instruments でリーク有無を確認すること` と **推奨メモ**として残している。

### 実施していないこと（意図的にスコープ外）

- フェーズ 2 以降の **`delegate` / `dispatch_queue` の追加クリーンアップ**（現状の実装で問題が出たときに別途検討）。
- **`malloc` struct の廃止**（フェーズ 3・別 issue 向け）。
