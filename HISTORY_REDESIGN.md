# 履歴システム再設計 - 実装方針

## 目標

すべての操作（描画、レイヤー操作、フレーム操作、キャンバスリサイズ、インポート等）を履歴管理対象とし、UIから任意の時点にロールバックできるようにする。

---

## 現状の問題

- `PixelChange`（ピクセル単位の差分）しか記録できない
- キャンバスリサイズ、レイヤー追加/削除、フレーム操作などは `History::new()` で履歴リセットされる
- フレーム切替のたびに履歴が失われる

## 推奨方式: スナップショット方式

### 基本設計

```rust
/// 履歴エントリ: 操作名 + その時点の完全な状態
struct Snapshot {
    label: String,              // "Draw", "Add Layer", "Resize 64x64" 等
    canvas: Canvas,             // 全レイヤー含むキャンバス状態
    timeline: Timeline,         // 全フレーム
    active_layer: usize,
    current_frame: usize,
}

/// スナップショットベースの履歴マネージャ
struct SnapshotHistory {
    snapshots: Vec<Snapshot>,   // 時系列順
    current: usize,             // 現在位置（0 = 最古）
    max_snapshots: usize,       // 上限（30〜50推奨）
}
```

### API

```rust
impl SnapshotHistory {
    /// 操作前に呼ぶ。現在の状態を保存する。
    fn push(&mut self, label: &str, canvas: &Canvas, timeline: &Timeline);

    /// Undo: current を1つ戻し、その時点の状態を返す
    fn undo(&mut self) -> Option<&Snapshot>;

    /// Redo: current を1つ進め、その時点の状態を返す
    fn redo(&mut self) -> Option<&Snapshot>;

    /// 任意の時点にジャンプ（履歴UIから選択時）
    fn jump_to(&mut self, index: usize) -> Option<&Snapshot>;

    /// 履歴一覧を返す（UI表示用）
    fn entries(&self) -> &[Snapshot];

    /// 現在位置
    fn current_index(&self) -> usize;
}
```

### 状態復元の流れ

```rust
// Undo/Redo/Jump 時
if let Some(snapshot) = history.undo() {  // or redo() / jump_to(i)
    state.canvas = snapshot.canvas.clone();
    state.timeline = snapshot.timeline.clone();
    state.canvas.active_layer = snapshot.active_layer;
    state.timeline.current_frame = snapshot.current_frame;
}
```

---

## 対象操作一覧

| 操作 | ラベル例 | 備考 |
|---|---|---|
| ペンシル/消しゴム描画 | `"Draw"` / `"Erase"` | mouseup 時に記録 |
| 塗りつぶし | `"Fill"` | 即座に記録 |
| 線/矩形/楕円 | `"Line"` / `"Rectangle"` 等 | mouseup 時に記録 |
| ペースト | `"Paste"` | |
| カット | `"Cut"` | |
| レイヤー追加 | `"Add Layer"` | |
| レイヤー削除 | `"Remove Layer"` | |
| レイヤー並べ替え | `"Move Layer"` | |
| レイヤー表示切替 | `"Toggle Layer Visibility"` | |
| レイヤー不透明度変更 | `"Layer Opacity"` | |
| フレーム追加 | `"Add Frame"` | |
| フレーム削除 | `"Remove Frame"` | |
| フレーム複製 | `"Duplicate Frame"` | |
| キャンバスリサイズ | `"Resize 64x64"` | |
| キャンバス新規作成 | `"New Canvas"` | 履歴クリア |
| PNG インポート | `"Import PNG"` | 履歴クリア |
| AI ピクセル化 | `"Pixelize"` | |
| パレット適用 | `"Apply Palette"` | |

---

## メモリ管理

### 見積もり

- 32x32 キャンバス, 1レイヤー: `32*32*4 = 4KB`
- 64x64 キャンバス, 3レイヤー: `64*64*4*3 = 48KB`
- 128x128 キャンバス, 3レイヤー, 5フレーム: `128*128*4*3*5 = 960KB`

### 制限

```rust
const MAX_SNAPSHOTS: usize = 50;
const MAX_MEMORY_MB: usize = 64;  // 超えたら古いスナップショットから破棄
```

- 32x32 の場合: 50スナップショット ≈ 200KB（余裕）
- 128x128 × 3レイヤー × 5フレームの場合: 50スナップショット ≈ 48MB（制限で調整）

### 最適化オプション（将来）

- **差分圧縮**: 前のスナップショットとの差分のみ保存（実装複雑だがメモリ大幅削減）
- **遅延クローン**: `Rc<Canvas>` + Copy-on-Write で未変更レイヤーを共有

---

## UI設計

### 履歴パネル

```
┌─ History ─────────────┐
│ ● New Canvas          │  ← 最古（薄い色）
│ ● Draw                │
│ ● Draw                │
│ ● Fill                │
│ ● Add Layer           │
│ ▶ Draw               │  ← 現在位置（ハイライト）
│ ○ Draw                │  ← Redo可能（グレーアウト）
│ ○ Resize 64x64        │
└───────────────────────┘
```

- `●` = Undo可能な過去の操作
- `▶` = 現在の状態
- `○` = Redo可能な未来の操作（新しい操作をするとクリア）
- クリックで任意の時点にジャンプ

### 配置

右パネル（レイヤーパネルの下、またはタブ切替）に配置。

---

## 実装手順

### Phase 1: コア実装

1. `crates/core/src/snapshot_history.rs` を新規作成
   - `Snapshot`, `SnapshotHistory` 構造体
   - `push`, `undo`, `redo`, `jump_to`, `entries` メソッド
   - メモリ上限による自動破棄
2. テスト追加

### Phase 2: EditorState 統合

1. `EditorState` の `history: History` を `history: SnapshotHistory` に置き換え
2. 既存の `History`（PixelChange ベース）は削除
3. 全操作箇所で `history.push(label, &canvas, &timeline)` を呼ぶ
4. `on_undo` / `on_redo` をスナップショット復元に変更

### Phase 3: 全操作への対応

1. `canvas_view.rs`: 描画ツール（mouseup / fill 完了時）
2. `main.rs`: レイヤー操作、フレーム操作、リサイズ、インポート、AI 処理
3. `state.rs`: フレーム切替で履歴リセットしないように変更

### Phase 4: 履歴UI

1. `components/history_panel.rs` 新規作成
2. 履歴一覧の表示（操作名リスト）
3. クリックで `jump_to` 呼び出し
4. 右パネルに配置

### Phase 5: 永続化（オプション）

- スナップショット方式の履歴はサイズが大きいため、localStorage への永続化は非推奨
- セッション内のみの履歴管理とし、最新の canvas/timeline のみ autosave する
- 既存の autosave（canvas + pixel-diff history）は最新状態の保存に特化

---

## 既存コードへの影響

- `crates/core/src/history.rs` → 削除または非推奨化
- `crates/tools/src/lib.rs` → `Command` / `PixelChange` パラメータを全ツール関数から削除可能
  - ツール関数はキャンバスを直接変更するだけになり、シンプルに
- `app/src/storage.rs` → 履歴の永続化を削除（canvas のみ保存に戻す）
- `app/src/main.rs` → 全操作で `history.push()` を追加
- `app/src/state.rs` → フレーム操作での `History::new()` を削除
